#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod crypto;
mod discovery;
mod input;
mod log;
mod platform;
mod proto;
mod server;

use std::sync::{Mutex, MutexGuard};

/// Trava ignorando envenenamento: um pânico numa conexão não pode travar o resto.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn main() {
    platform::main();
}
