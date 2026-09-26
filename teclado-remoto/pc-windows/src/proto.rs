//! Formato das mensagens do protocolo TRK1 (ver docs/PROTOCOLO.md).

pub const TCP_PORT: u16 = 47800;
pub const TCP_PORT_FALLBACKS: u16 = 10;
pub const DISCOVERY_PORT: u16 = 47810;
pub const MAGIC: &[u8; 4] = b"TRK1";
pub const VERSION: u8 = 1;

pub const MODE_SESSION: u8 = 1;
pub const MODE_PAIR: u8 = 2;

pub const ST_OK: u8 = 0;
pub const ST_NOT_PAIRED: u8 = 1;
pub const ST_PAIRING_DISABLED: u8 = 2;
pub const ST_BUSY: u8 = 3;
pub const ST_BAD_VERSION: u8 = 4;
pub const ST_REJECTED: u8 = 5;
pub const ST_BAD_REQUEST: u8 = 6;
pub const ST_TIMEOUT: u8 = 7;

pub const ID_LEN: usize = 16;
pub const PUB_LEN: usize = 65;
pub const NONCE_LEN: usize = 32;
pub const MAX_NAME: usize = 64;
pub const MAX_HANDSHAKE_FRAME: usize = 512;
pub const MAX_PLAINTEXT: usize = 1 << 20;
pub const TAG_LEN: usize = 16;

pub const MSG_TYPE: u8 = 0x01;
pub const MSG_KEY: u8 = 0x02;
pub const MSG_CHAR: u8 = 0x03;
pub const MSG_RELEASE_ALL: u8 = 0x04;
pub const MSG_CLIP_SET: u8 = 0x10;
pub const MSG_CLIP_GET: u8 = 0x11;
pub const MSG_CLIP_PASTE: u8 = 0x12;
pub const MSG_MOUSE_MOVE: u8 = 0x20;
pub const MSG_MOUSE_BUTTON: u8 = 0x21;
pub const MSG_MOUSE_WHEEL: u8 = 0x22;
pub const MSG_ACTION: u8 = 0x30;
pub const MSG_PING: u8 = 0x40;
pub const MSG_BYE: u8 = 0x41;

pub const MSG_WELCOME: u8 = 0x80;
pub const MSG_PONG: u8 = 0x81;
pub const MSG_CLIP_DATA: u8 = 0x82;
pub const MSG_NOTICE: u8 = 0x83;

pub const KEY_TAP: u8 = 0;
pub const KEY_DOWN: u8 = 1;
pub const KEY_UP: u8 = 2;

pub const MOD_CTRL: u8 = 1;
pub const MOD_SHIFT: u8 = 2;
pub const MOD_ALT: u8 = 4;
pub const MOD_WIN: u8 = 8;

pub const ACTION_LOCK: u8 = 1;

pub const WELCOME_FLAG_ELEVATED: u8 = 1;
pub const NOTICE_INPUT_BLOCKED: u8 = 1;

pub const PROBE_PREFIX: &[u8; 5] = b"TRK1?";
pub const REPLY_PREFIX: &[u8; 5] = b"TRK1!";

const HELLO_FIXED: usize = 120;

#[derive(Debug, Clone, PartialEq)]
pub struct ClientHello {
    pub mode: u8,
    pub device_id: [u8; ID_LEN],
    pub public: [u8; PUB_LEN],
    pub nonce: [u8; NONCE_LEN],
    pub name: String,
}

#[derive(Debug, PartialEq)]
pub enum HelloError {
    Malformed,
    BadVersion,
}

impl ClientHello {
    pub fn parse(b: &[u8]) -> Result<Self, HelloError> {
        if b.len() < 6 || &b[..4] != MAGIC {
            return Err(HelloError::Malformed);
        }
        if b[4] != VERSION {
            return Err(HelloError::BadVersion);
        }
        if b.len() < HELLO_FIXED {
            return Err(HelloError::Malformed);
        }
        let mode = b[5];
        if mode != MODE_SESSION && mode != MODE_PAIR {
            return Err(HelloError::Malformed);
        }
        let name_len = b[119] as usize;
        if name_len > MAX_NAME || b.len() != HELLO_FIXED + name_len {
            return Err(HelloError::Malformed);
        }
        let name = std::str::from_utf8(&b[HELLO_FIXED..]).map_err(|_| HelloError::Malformed)?;
        Ok(ClientHello {
            mode,
            device_id: b[6..22].try_into().unwrap(),
            public: b[22..87].try_into().unwrap(),
            nonce: b[87..119].try_into().unwrap(),
            name: sanitize_name(name),
        })
    }

    #[cfg(test)]
    pub fn encode(&self) -> Vec<u8> {
        let name = truncate_utf8(&self.name, MAX_NAME);
        let mut out = Vec::with_capacity(HELLO_FIXED + name.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(self.mode);
        out.extend_from_slice(&self.device_id);
        out.extend_from_slice(&self.public);
        out.extend_from_slice(&self.nonce);
        out.push(name.len() as u8);
        out.extend_from_slice(name.as_bytes());
        out
    }
}

pub struct ServerHello<'a> {
    pub server_id: &'a [u8; ID_LEN],
    pub public: &'a [u8; PUB_LEN],
    pub nonce_or_commit: &'a [u8; NONCE_LEN],
    pub name: &'a str,
}

impl ServerHello<'_> {
    pub fn encode(&self) -> Vec<u8> {
        let name = truncate_utf8(self.name, MAX_NAME);
        let mut out = Vec::with_capacity(HELLO_FIXED + name.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(ST_OK);
        out.extend_from_slice(self.server_id);
        out.extend_from_slice(self.public);
        out.extend_from_slice(self.nonce_or_commit);
        out.push(name.len() as u8);
        out.extend_from_slice(name.as_bytes());
        out
    }
}

pub fn server_status(status: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(6);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(status);
    out
}

#[derive(Debug, PartialEq)]
pub enum ClientMsg<'a> {
    Type { backspaces: u16, text: &'a str },
    Key { action: u8, mods: u8, vk: u16 },
    Char { mods: u8, codepoint: u32 },
    ReleaseAll,
    ClipSet(&'a str),
    ClipGet,
    ClipPaste(&'a str),
    MouseMove { dx: i16, dy: i16 },
    MouseButton { button: u8, action: u8 },
    MouseWheel { vertical: i16, horizontal: i16 },
    Action(u8),
    Ping([u8; 8]),
    Bye,
    Unknown(u8),
}

fn be16(b: &[u8]) -> u16 {
    u16::from_be_bytes([b[0], b[1]])
}

impl<'a> ClientMsg<'a> {
    /// `None` = mensagem malformada (tamanho errado ou UTF-8 inválido).
    pub fn parse(b: &'a [u8]) -> Option<Self> {
        let (&kind, body) = b.split_first()?;
        let text = |bytes: &'a [u8]| std::str::from_utf8(bytes).ok();
        Some(match kind {
            MSG_TYPE if body.len() >= 2 => ClientMsg::Type { backspaces: be16(body), text: text(&body[2..])? },
            MSG_KEY if body.len() == 4 => ClientMsg::Key { action: body[0], mods: body[1], vk: be16(&body[2..]) },
            MSG_CHAR if body.len() == 5 => {
                ClientMsg::Char { mods: body[0], codepoint: u32::from_be_bytes(body[1..5].try_into().unwrap()) }
            }
            MSG_RELEASE_ALL => ClientMsg::ReleaseAll,
            MSG_CLIP_SET => ClientMsg::ClipSet(text(body)?),
            MSG_CLIP_GET => ClientMsg::ClipGet,
            MSG_CLIP_PASTE => ClientMsg::ClipPaste(text(body)?),
            MSG_MOUSE_MOVE if body.len() == 4 => {
                ClientMsg::MouseMove { dx: be16(body) as i16, dy: be16(&body[2..]) as i16 }
            }
            MSG_MOUSE_BUTTON if body.len() == 2 => ClientMsg::MouseButton { button: body[0], action: body[1] },
            MSG_MOUSE_WHEEL if body.len() == 4 => {
                ClientMsg::MouseWheel { vertical: be16(body) as i16, horizontal: be16(&body[2..]) as i16 }
            }
            MSG_ACTION if body.len() == 1 => ClientMsg::Action(body[0]),
            MSG_PING if body.len() == 8 => ClientMsg::Ping(body.try_into().unwrap()),
            MSG_BYE => ClientMsg::Bye,
            MSG_TYPE | MSG_KEY | MSG_CHAR | MSG_MOUSE_MOVE | MSG_MOUSE_BUTTON | MSG_MOUSE_WHEEL | MSG_ACTION
            | MSG_PING => return None,
            other => ClientMsg::Unknown(other),
        })
    }
}

pub fn welcome(flags: u8, name: &str) -> Vec<u8> {
    let mut out = vec![MSG_WELCOME, flags];
    out.extend_from_slice(truncate_utf8(name, MAX_NAME).as_bytes());
    out
}

pub fn pong(payload: &[u8; 8]) -> Vec<u8> {
    let mut out = vec![MSG_PONG];
    out.extend_from_slice(payload);
    out
}

pub fn clip_data(text: &str) -> Vec<u8> {
    let text = truncate_utf8(text, MAX_PLAINTEXT - 1);
    let mut out = Vec::with_capacity(1 + text.len());
    out.push(MSG_CLIP_DATA);
    out.extend_from_slice(text.as_bytes());
    out
}

pub fn notice(code: u8, text: &str) -> Vec<u8> {
    let mut out = vec![MSG_NOTICE, code];
    out.extend_from_slice(text.as_bytes());
    out
}

pub fn parse_probe(b: &[u8]) -> Option<[u8; 4]> {
    if b.len() == 9 && &b[..5] == PROBE_PREFIX {
        Some(b[5..9].try_into().unwrap())
    } else {
        None
    }
}

pub fn discovery_reply(probe_id: &[u8; 4], server_id: &[u8; ID_LEN], port: u16, name: &str) -> Vec<u8> {
    let name = truncate_utf8(name, MAX_NAME);
    let mut out = Vec::with_capacity(29 + name.len());
    out.extend_from_slice(REPLY_PREFIX);
    out.extend_from_slice(probe_id);
    out.push(VERSION);
    out.extend_from_slice(server_id);
    out.extend_from_slice(&port.to_be_bytes());
    out.push(name.len() as u8);
    out.extend_from_slice(name.as_bytes());
    out
}

/// Corta sem quebrar um caractere UTF-8 no meio.
pub fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

pub fn sanitize_name(s: &str) -> String {
    let clean: String = s.chars().filter(|c| !c.is_control()).collect();
    let trimmed = clean.trim();
    let name = if trimmed.is_empty() { "Celular" } else { trimmed };
    truncate_utf8(name, MAX_NAME).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello(name: &str) -> ClientHello {
        ClientHello { mode: MODE_PAIR, device_id: [7; 16], public: [4; 65], nonce: [0; 32], name: name.to_string() }
    }

    #[test]
    fn client_hello_roundtrip() {
        let h = hello("Galaxy S23 do João");
        assert_eq!(ClientHello::parse(&h.encode()).unwrap(), h);
    }

    #[test]
    fn client_hello_rejects_bad_input() {
        let mut b = hello("x").encode();
        b[4] = 2;
        assert_eq!(ClientHello::parse(&b), Err(HelloError::BadVersion));
        let mut b = hello("x").encode();
        b[5] = 9;
        assert_eq!(ClientHello::parse(&b), Err(HelloError::Malformed));
        let mut b = hello("abc").encode();
        b.pop();
        assert_eq!(ClientHello::parse(&b), Err(HelloError::Malformed));
        let mut b = hello("ab").encode();
        let n = b.len();
        b[n - 1] = 0xFF;
        assert_eq!(ClientHello::parse(&b), Err(HelloError::Malformed));
        assert_eq!(ClientHello::parse(b"GET / HTTP/1.1\r\n"), Err(HelloError::Malformed));
    }

    #[test]
    fn parses_client_messages() {
        let mut t = vec![MSG_TYPE, 0, 3];
        t.extend_from_slice("olá\n".as_bytes());
        assert_eq!(ClientMsg::parse(&t), Some(ClientMsg::Type { backspaces: 3, text: "olá\n" }));
        assert_eq!(
            ClientMsg::parse(&[MSG_KEY, KEY_TAP, MOD_CTRL, 0, 0x43]),
            Some(ClientMsg::Key { action: KEY_TAP, mods: MOD_CTRL, vk: 0x43 })
        );
        assert_eq!(
            ClientMsg::parse(&[MSG_CHAR, MOD_CTRL, 0, 0, 0, b'c']),
            Some(ClientMsg::Char { mods: MOD_CTRL, codepoint: 'c' as u32 })
        );
        assert_eq!(ClientMsg::parse(&[MSG_MOUSE_MOVE, 0xFF, 0xFE, 0, 5]), Some(ClientMsg::MouseMove { dx: -2, dy: 5 }));
        assert_eq!(ClientMsg::parse(&[MSG_KEY, 0, 0]), None);
        assert_eq!(ClientMsg::parse(&[MSG_TYPE, 0, 0, 0xC3]), None);
        assert_eq!(ClientMsg::parse(&[0x7E, 1, 2]), Some(ClientMsg::Unknown(0x7E)));
        assert_eq!(ClientMsg::parse(&[]), None);
    }

    #[test]
    fn names_are_sanitized_and_truncated() {
        assert_eq!(sanitize_name("  a\u{0}b\nc  "), "abc");
        assert_eq!(sanitize_name("   "), "Celular");
        let long = "é".repeat(40);
        let s = sanitize_name(&long);
        assert!(s.len() <= MAX_NAME && s.is_char_boundary(s.len()));
    }

    #[test]
    fn probe_and_reply() {
        assert_eq!(parse_probe(b"TRK1?abcd"), Some(*b"abcd"));
        assert_eq!(parse_probe(b"TRK1?abc"), None);
        let r = discovery_reply(b"abcd", &[1; 16], 47800, "PC");
        assert_eq!(&r[..9], b"TRK1!abcd");
        assert_eq!(r[9], VERSION);
        assert_eq!(&r[26..28], &47800u16.to_be_bytes());
        assert_eq!(&r[29..], b"PC");
    }
}
