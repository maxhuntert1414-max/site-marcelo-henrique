mod backend;
mod sys;
mod ui;

use std::sync::Arc;

pub use sys::{boost_thread_priority, protect, unprotect};

use crate::config::Config;
use crate::discovery;
use crate::proto::DISCOVERY_PORT;
use crate::server::{self, Shared};

pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--configure-firewall") {
        sys::configure_firewall();
        return;
    }

    let dir = sys::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    crate::log::init(&dir);
    if !sys::acquire_single_instance(has("--restarted")) {
        ui::activate_existing();
        return;
    }
    crate::log!(
        "Teclado Remoto {} iniciado{}",
        env!("CARGO_PKG_VERSION"),
        if sys::is_elevated() { " como administrador" } else { "" }
    );
    sys::refresh_autostart_path();

    let config = Config::load(&dir, &sys::computer_name());
    let shared = Shared::new(config, Arc::new(backend::WinBackend::new()), Arc::new(ui::WinUi));
    let port_ok = match server::start(&shared) {
        Ok(_) => true,
        Err(e) => {
            crate::log!("não consegui abrir a porta TCP: {e}");
            false
        }
    };
    discovery::start(&shared, DISCOVERY_PORT);
    ui::run(shared, port_ok, has("--tray"));
}
