//! Responde às sondas UDP do celular para ele achar este PC sem digitar IP.

use std::net::{Ipv4Addr, UdpSocket};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::lock;
use crate::proto;
use crate::server::Shared;

const MAX_REPLIES_PER_SECOND: u32 = 20;

pub fn start(shared: &Arc<Shared>, port: u16) {
    let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)) {
        Ok(s) => s,
        Err(e) => {
            crate::log!("descoberta automática indisponível (UDP {port}): {e}");
            return;
        }
    };
    let shared = shared.clone();
    let spawned = thread::Builder::new().name("discovery".into()).spawn(move || {
        let mut buf = [0u8; 64];
        let mut window = Instant::now();
        let mut replies = 0;
        loop {
            // No Windows, um ICMP "porta inalcançável" aparece aqui como erro; só seguir em frente.
            let (len, from) = match socket.recv_from(&mut buf) {
                Ok(r) => r,
                Err(_) => {
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
            };
            let Some(probe) = proto::parse_probe(&buf[..len]) else { continue };
            if window.elapsed() >= Duration::from_secs(1) {
                window = Instant::now();
                replies = 0;
            }
            replies += 1;
            if replies > MAX_REPLIES_PER_SECOND {
                continue;
            }
            let (id, name) = {
                let cfg = lock(&shared.config);
                (cfg.server_id, cfg.name.clone())
            };
            let reply = proto::discovery_reply(&probe, &id, shared.port.load(Ordering::Relaxed), &name);
            let _ = socket.send_to(&reply, from);
        }
    });
    if let Err(e) = spawned {
        crate::log!("não consegui iniciar a descoberta: {e}");
    }
}
