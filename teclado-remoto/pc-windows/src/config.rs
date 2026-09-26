//! Configuração e celulares pareados, salvos em texto simples na pasta do usuário.
//! As chaves de pareamento são protegidas pelo sistema (DPAPI no Windows).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};

use crate::crypto::{from_hex, random_bytes, to_hex};
use crate::platform;
use crate::proto::{sanitize_name, truncate_utf8, ID_LEN, MAX_NAME, TCP_PORT};

#[derive(Clone)]
pub struct Device {
    pub id: [u8; ID_LEN],
    pub name: String,
    pub key: [u8; 32],
    pub paired_at: u64,
    pub last_seen: u64,
}

pub struct Config {
    pub server_id: [u8; ID_LEN],
    pub name: String,
    pub port: u16,
    pub allow_pairing: bool,
    pub devices: Vec<Device>,
    path: PathBuf,
}

pub fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Config {
    pub fn load(dir: &Path, default_name: &str) -> Config {
        let mut cfg = Config {
            server_id: random_bytes(),
            name: truncate_utf8(default_name, MAX_NAME).to_string(),
            port: TCP_PORT,
            allow_pairing: true,
            devices: Vec::new(),
            path: dir.join("config.txt"),
        };
        match fs::read_to_string(&cfg.path) {
            Ok(text) => cfg.parse(&text),
            Err(_) => {
                if let Err(e) = cfg.save() {
                    crate::log!("não consegui criar {}: {e}", cfg.path.display());
                }
            }
        }
        cfg
    }

    fn parse(&mut self, text: &str) {
        for line in text.lines() {
            let Some((key, value)) = line.trim_end_matches('\r').split_once('=') else {
                continue;
            };
            match key {
                "server_id" => {
                    if let Some(id) = from_hex(value).and_then(|b| b.try_into().ok()) {
                        self.server_id = id;
                    }
                }
                "name" if !value.trim().is_empty() => self.name = sanitize_name(value),
                "port" => {
                    if let Ok(p) = value.parse::<u16>() {
                        if p >= 1024 {
                            self.port = p;
                        }
                    }
                }
                "allow_pairing" => self.allow_pairing = value.trim() != "0",
                "device" => match parse_device(value) {
                    Some(d) => self.upsert(d),
                    None => crate::log!("celular pareado ignorado (dados ilegíveis)"),
                },
                _ => {}
            }
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let mut out = String::from("# Teclado Remoto — configuração. Feche o programa antes de editar.\n");
        out += &format!("server_id={}\n", to_hex(&self.server_id));
        out += &format!("name={}\n", self.name);
        out += &format!("port={}\n", self.port);
        out += &format!("allow_pairing={}\n", if self.allow_pairing { 1 } else { 0 });
        for d in &self.devices {
            out += &format!(
                "device={} {} {} {} {}\n",
                to_hex(&d.id),
                to_hex(&platform::protect(&d.key)),
                d.paired_at,
                d.last_seen,
                d.name
            );
        }
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, out)?;
        fs::rename(&tmp, &self.path)
    }

    pub fn find(&self, id: &[u8; ID_LEN]) -> Option<&Device> {
        self.devices.iter().find(|d| &d.id == id)
    }

    pub fn upsert(&mut self, device: Device) {
        match self.devices.iter_mut().find(|d| d.id == device.id) {
            Some(existing) => *existing = device,
            None => self.devices.push(device),
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn remove(&mut self, id: &[u8; ID_LEN]) -> bool {
        let before = self.devices.len();
        self.devices.retain(|d| &d.id != id);
        before != self.devices.len()
    }

    pub fn touch(&mut self, id: &[u8; ID_LEN], name: &str) {
        if let Some(d) = self.devices.iter_mut().find(|d| &d.id == id) {
            d.last_seen = now_unix();
            d.name = name.to_string();
        }
    }
}

fn parse_device(value: &str) -> Option<Device> {
    let mut parts = value.splitn(5, ' ');
    let id = from_hex(parts.next()?)?.try_into().ok()?;
    let key = platform::unprotect(&from_hex(parts.next()?)?)?.try_into().ok()?;
    let paired_at = parts.next()?.parse().ok()?;
    let last_seen = parts.next()?.parse().ok()?;
    let name = sanitize_name(parts.next().unwrap_or(""));
    Some(Device { id, name, key, paired_at, last_seen })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_devices() {
        let dir = std::env::temp_dir().join(format!("trk-cfg-{}", to_hex(&random_bytes::<6>())));
        let mut cfg = Config::load(&dir, "MEU-PC");
        assert_eq!(cfg.name, "MEU-PC");
        cfg.allow_pairing = false;
        cfg.upsert(Device { id: [1; 16], name: "Galaxy do Zé".into(), key: [2; 32], paired_at: 10, last_seen: 20 });
        cfg.save().unwrap();

        let loaded = Config::load(&dir, "OUTRO");
        assert_eq!(loaded.server_id, cfg.server_id);
        assert_eq!(loaded.name, "MEU-PC");
        assert!(!loaded.allow_pairing);
        let d = loaded.find(&[1; 16]).unwrap();
        assert_eq!(d.name, "Galaxy do Zé");
        assert_eq!(d.key, [2; 32]);
        assert_eq!((d.paired_at, d.last_seen), (10, 20));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn upsert_replaces_and_remove_deletes() {
        let dir = std::env::temp_dir().join(format!("trk-cfg-{}", to_hex(&random_bytes::<6>())));
        let mut cfg = Config::load(&dir, "PC");
        let dev = |name: &str| Device { id: [5; 16], name: name.into(), key: [0; 32], paired_at: 0, last_seen: 0 };
        cfg.upsert(dev("a"));
        cfg.upsert(dev("b"));
        assert_eq!(cfg.devices.len(), 1);
        assert_eq!(cfg.devices[0].name, "b");
        assert!(cfg.remove(&[5; 16]));
        assert!(!cfg.remove(&[5; 16]));
        fs::remove_dir_all(&dir).unwrap();
    }
}
