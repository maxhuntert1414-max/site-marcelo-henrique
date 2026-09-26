//! Injeção de teclado/mouse com SendInput.

use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::Mutex;

use windows_sys::Win32::System::Shutdown::LockWorkStation;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, MapVirtualKeyW, SendInput, VkKeyScanExW, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE,
    KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC_EX, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use super::{sys, ui};
use crate::input::{Backend, Raw};
use crate::lock;
use crate::proto::{MOD_ALT, MOD_CTRL, MOD_SHIFT};

/// Marca nossos eventos ("TRK1") para quem quiser distingui-los de um teclado físico.
const EXTRA_INFO: usize = 0x5452_4B31;
const VK_PAUSE: u16 = 0x13;

/// Teclas que o Windows só entende direito com a flag "estendida".
fn is_extended(vk: u16) -> bool {
    matches!(
        vk,
        0x21..=0x28      // PgUp, PgDn, End, Home, setas
            | 0x2C..=0x2E // PrintScreen, Insert, Delete
            | 0x5B..=0x5D // Win esquerda/direita, Menu de contexto
            | 0x6F        // / do teclado numérico
            | 0x90        // NumLock
            | 0xA3        // Ctrl direito
            | 0xA5        // Alt direito (AltGr)
            | 0xA6..=0xB7 // navegador, volume e mídia
    )
}

fn key_input(vk: u16, up: bool) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC_EX) };
    let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
    if is_extended(vk) || scan & 0xFF00 == 0xE000 {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    let scan = if vk == VK_PAUSE { 0x45 } else { (scan & 0xFF) as u16 };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: EXTRA_INFO },
        },
    }
}

fn unicode_input(unit: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: EXTRA_INFO,
            },
        },
    }
}

fn mouse_input(dx: i32, dy: i32, data: i32, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT { dx, dy, mouseData: data as u32, dwFlags: flags, time: 0, dwExtraInfo: EXTRA_INFO },
        },
    }
}

fn to_input(raw: &Raw) -> INPUT {
    match *raw {
        Raw::Key { vk, up } => key_input(vk, up),
        Raw::Unicode { unit, up } => unicode_input(unit, up),
        Raw::MouseMove { dx, dy } => mouse_input(dx, dy, 0, MOUSEEVENTF_MOVE),
        Raw::MouseButton { button, up } => {
            let flags = match (button, up) {
                (1, false) => MOUSEEVENTF_LEFTDOWN,
                (1, true) => MOUSEEVENTF_LEFTUP,
                (2, false) => MOUSEEVENTF_RIGHTDOWN,
                (2, true) => MOUSEEVENTF_RIGHTUP,
                (_, false) => MOUSEEVENTF_MIDDLEDOWN,
                (_, true) => MOUSEEVENTF_MIDDLEUP,
            };
            mouse_input(0, 0, 0, flags)
        }
        Raw::Wheel { delta, horizontal } => {
            mouse_input(0, 0, delta, if horizontal { MOUSEEVENTF_HWHEEL } else { MOUSEEVENTF_WHEEL })
        }
    }
}

pub struct WinBackend {
    elevated: bool,
    /// Última janela verificada (endereço, bloqueada?) para não repetir a consulta a cada tecla.
    foreground: Mutex<(usize, bool)>,
}

impl WinBackend {
    pub fn new() -> Self {
        WinBackend { elevated: sys::is_elevated(), foreground: Mutex::new((0, false)) }
    }
}

impl Backend for WinBackend {
    fn send(&self, inputs: &[Raw]) {
        let batch: Vec<INPUT> = inputs.iter().map(to_input).collect();
        unsafe {
            SendInput(batch.len() as u32, batch.as_ptr(), size_of::<INPUT>() as i32);
        }
    }

    fn resolve_char(&self, ch: u32) -> Option<(u16, u8)> {
        let unit = u16::try_from(ch).ok()?;
        let result = unsafe {
            let thread = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
            VkKeyScanExW(unit, GetKeyboardLayout(thread))
        };
        if result == -1 {
            return None;
        }
        let vk = (result as u16) & 0xFF;
        let state = ((result as u16) >> 8) as u8;
        if state & !0x07 != 0 {
            return None;
        }
        let mut mods = 0;
        if state & 1 != 0 {
            mods |= MOD_SHIFT;
        }
        if state & 2 != 0 {
            mods |= MOD_CTRL;
        }
        if state & 4 != 0 {
            mods |= MOD_ALT;
        }
        Some((vk, mods))
    }

    fn clipboard_get(&self) -> Option<String> {
        ui::clipboard_get()
    }

    fn clipboard_set(&self, text: &str) -> bool {
        ui::clipboard_set(text)
    }

    fn lock_workstation(&self) {
        unsafe {
            LockWorkStation();
        }
    }

    fn input_blocked(&self) -> bool {
        if self.elevated {
            return false;
        }
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() {
            return false;
        }
        let mut cache = lock(&self.foreground);
        if cache.0 != hwnd as usize {
            let mut pid = 0u32;
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut pid);
            }
            *cache = (hwnd as usize, pid != 0 && sys::process_elevated(pid));
        }
        cache.1
    }

    fn is_elevated(&self) -> bool {
        self.elevated
    }
}
