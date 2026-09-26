//! ECDH P-256 + HKDF-SHA256 + AES-256-GCM, exatamente como em docs/PROTOCOLO.md.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use p256::ecdh::EphemeralSecret;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::PublicKey;
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::proto::{NONCE_LEN, PUB_LEN};

pub const DIR_C2S: u8 = 1;
pub const DIR_S2C: u8 = 2;

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    OsRng.fill_bytes(&mut b);
    b
}

pub fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

fn hkdf(salt: &[u8], ikm: &[u8], info: &[&[u8]], out: &mut [u8]) {
    Hkdf::<Sha256>::new(Some(salt), ikm).expand_multi_info(info, out).expect("tamanho de saída do HKDF válido");
}

pub struct Ephemeral {
    secret: EphemeralSecret,
    pub public: [u8; PUB_LEN],
}

impl Ephemeral {
    pub fn generate() -> Self {
        let secret = EphemeralSecret::random(&mut OsRng);
        let point = secret.public_key().to_encoded_point(false);
        let public = point.as_bytes().try_into().expect("ponto SEC1 sem compressão tem 65 bytes");
        Ephemeral { secret, public }
    }

    /// Segredo compartilhado (coordenada X). `None` se a chave do outro lado for inválida.
    pub fn agree(&self, peer: &[u8; PUB_LEN]) -> Option<[u8; 32]> {
        let peer = PublicKey::from_sec1_bytes(peer).ok()?;
        let shared = self.secret.diffie_hellman(&peer);
        Some((*shared.raw_secret_bytes()).into())
    }
}

pub fn commitment(server_pub: &[u8], client_pub: &[u8], server_nonce: &[u8; NONCE_LEN]) -> [u8; 32] {
    sha256(&[b"TRK1-commit", server_pub, client_pub, server_nonce])
}

/// Código de 6 dígitos que o usuário compara no celular e no PC.
pub fn sas_code(client_pub: &[u8], server_pub: &[u8], nc: &[u8; NONCE_LEN], ns: &[u8; NONCE_LEN]) -> u32 {
    let h = sha256(&[b"TRK1-sas", client_pub, server_pub, nc, ns]);
    u32::from_be_bytes([h[0], h[1], h[2], h[3]]) % 1_000_000
}

pub fn format_sas(code: u32) -> String {
    format!("{:03} {:03}", code / 1000, code % 1000)
}

pub fn pair_key(transcript: &[u8; 32], shared: &[u8; 32]) -> [u8; 32] {
    let mut k = [0u8; 32];
    hkdf(transcript, shared, &[b"TRK1-pair"], &mut k);
    k
}

pub struct SessionKeys {
    pub c2s: [u8; 32],
    pub s2c: [u8; 32],
}

pub fn session_keys(pair_key: &[u8; 32], shared: &[u8; 32], transcript: &[u8; 32]) -> SessionKeys {
    let mut okm = [0u8; 64];
    hkdf(pair_key, shared, &[b"TRK1-session", transcript], &mut okm);
    SessionKeys { c2s: okm[..32].try_into().unwrap(), s2c: okm[32..].try_into().unwrap() }
}

/// Cifra de um sentido da conexão. O nonce é implícito (contador), então
/// moldura repetida, fora de ordem ou alterada simplesmente não abre.
pub struct FrameCipher {
    aead: Aes256Gcm,
    dir: u8,
    counter: u64,
}

impl FrameCipher {
    pub fn new(key: &[u8; 32], dir: u8) -> Self {
        FrameCipher { aead: Aes256Gcm::new(key.into()), dir, counter: 0 }
    }

    fn next_nonce(&mut self) -> [u8; 12] {
        let mut n = [0u8; 12];
        n[3] = self.dir;
        n[4..].copy_from_slice(&self.counter.to_be_bytes());
        self.counter += 1;
        n
    }

    pub fn seal(&mut self, plain: &[u8]) -> Vec<u8> {
        let nonce = self.next_nonce();
        self.aead.encrypt(Nonce::from_slice(&nonce), plain).expect("AES-GCM não falha ao cifrar")
    }

    pub fn open(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
        let nonce = self.next_nonce();
        self.aead.decrypt(Nonce::from_slice(&nonce), frame).ok()
    }
}

pub fn to_hex(b: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for &x in b {
        s.push(HEX[(x >> 4) as usize] as char);
        s.push(HEX[(x & 15) as usize] as char);
    }
    s
}

pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecdh_agrees_both_ways() {
        let a = Ephemeral::generate();
        let b = Ephemeral::generate();
        assert_eq!(a.public[0], 4);
        assert_eq!(a.agree(&b.public), b.agree(&a.public));
        assert!(a.agree(&[4; 65]).is_none());
    }

    #[test]
    fn frames_open_only_in_order_with_right_key() {
        let key = [9u8; 32];
        let mut tx = FrameCipher::new(&key, DIR_C2S);
        let mut rx = FrameCipher::new(&key, DIR_C2S);
        let f1 = tx.seal(b"um");
        let f2 = tx.seal(b"dois");
        assert_eq!(f1.len(), 2 + 16);
        assert_eq!(rx.open(&f1).as_deref(), Some(&b"um"[..]));
        assert_eq!(rx.open(&f2).as_deref(), Some(&b"dois"[..]));

        let mut replay = FrameCipher::new(&key, DIR_C2S);
        replay.open(&f1).unwrap();
        assert!(replay.open(&f1).is_none(), "repetição não pode abrir");

        let mut other_dir = FrameCipher::new(&key, DIR_S2C);
        assert!(other_dir.open(&f1).is_none());

        let mut tampered = f1.clone();
        tampered[0] ^= 1;
        assert!(FrameCipher::new(&key, DIR_C2S).open(&tampered).is_none());
    }

    #[test]
    fn hkdf_matches_rfc5869_case1() {
        let ikm = [0x0b; 22];
        let salt = from_hex("000102030405060708090a0b0c").unwrap();
        let info = from_hex("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let mut okm = [0u8; 42];
        hkdf(&salt, &ikm, &[&info[..5], &info[5..]], &mut okm);
        assert_eq!(
            to_hex(&okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    #[test]
    fn sas_is_six_digits_and_formatted() {
        let code = sas_code(&[1; 65], &[2; 65], &[3; 32], &[4; 32]);
        assert!(code < 1_000_000);
        assert_eq!(format_sas(42), "000 042");
        assert_eq!(format_sas(123456), "123 456");
    }

    #[test]
    fn hex_roundtrip() {
        assert_eq!(to_hex(&[0, 0xab, 0x10]), "00ab10");
        assert_eq!(from_hex("00ab10"), Some(vec![0, 0xab, 0x10]));
        assert_eq!(from_hex("0g"), None);
        assert_eq!(from_hex("abc"), None);
    }
}
