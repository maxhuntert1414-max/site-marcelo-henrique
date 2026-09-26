//! Modo sem interface para Linux/macOS: não digita nada, só imprime os eventos
//! que o Windows receberia. Serve para os testes ponta a ponta com o app Android.
//!
//! Variáveis: TRK_CONFIG_DIR, TRK_PORT, TRK_DISCOVERY_PORT e TRK_AUTO_ACCEPT=1
//! (aceita pareamentos sem perguntar — só existe neste modo de teste).

use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::{env, fs, thread};

use crate::config::Config;
use crate::crypto::format_sas;
use crate::discovery;
use crate::input::{Backend, Raw};
use crate::proto::{DISCOVERY_PORT, MOD_SHIFT};
use crate::server::{self, PairRequest, Shared, Ui};

pub fn protect(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

pub fn unprotect(blob: &[u8]) -> Option<Vec<u8>> {
    Some(blob.to_vec())
}

pub fn boost_thread_priority() {}

fn emit(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

#[derive(Default)]
struct PrintBackend {
    clipboard: Mutex<String>,
}

impl Backend for PrintBackend {
    fn send(&self, inputs: &[Raw]) {
        for input in inputs {
            let line = match *input {
                Raw::Key { vk, up } => format!("KEY 0x{vk:02X} {}", if up { "up" } else { "down" }),
                Raw::Unicode { unit, up } => format!("UNICODE U+{unit:04X} {}", if up { "up" } else { "down" }),
                Raw::MouseMove { dx, dy } => format!("MOUSE_MOVE {dx} {dy}"),
                Raw::MouseButton { button, up } => format!("MOUSE_BUTTON {button} {}", if up { "up" } else { "down" }),
                Raw::Wheel { delta, horizontal } => format!("WHEEL {delta} {}", if horizontal { "h" } else { "v" }),
            };
            emit(&line);
        }
    }

    fn resolve_char(&self, ch: u32) -> Option<(u16, u8)> {
        // Recorte do layout US, suficiente para os testes.
        let c = char::from_u32(ch)?;
        Some(match c {
            '0'..='9' | ' ' => (c as u16, 0),
            '-' => (0xBD, 0),
            '_' => (0xBD, MOD_SHIFT),
            '=' => (0xBB, 0),
            '+' => (0xBB, MOD_SHIFT),
            ',' => (0xBC, 0),
            '.' => (0xBE, 0),
            '/' => (0xBF, 0),
            '?' => (0xBF, MOD_SHIFT),
            '!' => (0x31, MOD_SHIFT),
            _ => return None,
        })
    }

    fn clipboard_get(&self) -> Option<String> {
        Some(crate::lock(&self.clipboard).clone())
    }

    fn clipboard_set(&self, text: &str) -> bool {
        emit(&format!("CLIPBOARD {text:?}"));
        *crate::lock(&self.clipboard) = text.to_string();
        true
    }

    fn lock_workstation(&self) {
        emit("LOCK");
    }

    fn input_blocked(&self) -> bool {
        false
    }

    fn is_elevated(&self) -> bool {
        false
    }
}

struct PrintUi {
    shared: std::sync::OnceLock<std::sync::Weak<Shared>>,
}

impl Ui for PrintUi {
    fn state_changed(&self) {
        if let Some(shared) = self.shared.get().and_then(|w| w.upgrade()) {
            let sessions = crate::lock(&shared.sessions);
            let list: Vec<String> = sessions.iter().map(|s| format!("{}@{}", s.device_name, s.addr.ip())).collect();
            emit(&format!("SESSIONS [{}]", list.join(", ")));
        }
    }

    fn ask_pairing(&self, request: PairRequest) -> mpsc::Receiver<bool> {
        emit(&format!("PAIR_REQUEST {} {} {}", format_sas(request.code), request.addr, request.device_name));
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(env::var("TRK_AUTO_ACCEPT").as_deref() == Ok("1"));
        rx
    }

    fn cancel_pairing(&self) {}

    fn paired(&self, device_name: &str) {
        emit(&format!("PAIRED {device_name}"));
    }
}

fn env_port(name: &str) -> Option<u16> {
    env::var(name).ok()?.parse().ok()
}

pub fn main() {
    let dir = env::var_os("TRK_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/teclado-remoto")))
        .unwrap_or_else(|| PathBuf::from("teclado-remoto"));
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("não consegui criar {}: {e}", dir.display());
        std::process::exit(1);
    }
    crate::log::init(&dir);
    let host = env::var("HOSTNAME").unwrap_or_else(|_| "Servidor de teste".into());
    let mut config = Config::load(&dir, &host);
    if let Some(port) = env_port("TRK_PORT") {
        config.port = port;
    }
    let ui = Arc::new(PrintUi { shared: std::sync::OnceLock::new() });
    let shared = Shared::new(config, Arc::new(PrintBackend::default()), ui.clone());
    let _ = ui.shared.set(Arc::downgrade(&shared));
    let port = match server::start(&shared) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("não consegui abrir a porta TCP: {e}");
            std::process::exit(1);
        }
    };
    discovery::start(&shared, env_port("TRK_DISCOVERY_PORT").unwrap_or(DISCOVERY_PORT));
    emit(&format!("READY {port}"));
    loop {
        thread::park();
    }
}
