//! Transforma mensagens do celular em eventos de teclado/mouse, sem depender do
//! sistema operacional. A plataforma só executa a lista pronta de eventos.

use crate::proto::{KEY_DOWN, KEY_TAP, KEY_UP, MOD_ALT, MOD_CTRL, MOD_SHIFT, MOD_WIN};

pub const VK_BACK: u16 = 0x08;
pub const VK_TAB: u16 = 0x09;
pub const VK_RETURN: u16 = 0x0D;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_CONTROL: u16 = 0x11;
pub const VK_MENU: u16 = 0x12;
pub const VK_LWIN: u16 = 0x5B;
pub const VK_V: u16 = 0x56;

const MAX_HELD: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raw {
    Key { vk: u16, up: bool },
    Unicode { unit: u16, up: bool },
    MouseMove { dx: i32, dy: i32 },
    MouseButton { button: u8, up: bool },
    Wheel { delta: i32, horizontal: bool },
}

pub trait Backend: Send + Sync {
    fn send(&self, inputs: &[Raw]);
    /// Tecla (vk) e modificadores necessários para produzir `ch` no layout atual do PC.
    fn resolve_char(&self, ch: u32) -> Option<(u16, u8)>;
    fn clipboard_get(&self) -> Option<String>;
    fn clipboard_set(&self, text: &str) -> bool;
    fn lock_workstation(&self);
    /// A janela em foco pertence a um processo com mais privilégio e vai ignorar a digitação.
    fn input_blocked(&self) -> bool;
    fn is_elevated(&self) -> bool;
}

const MODIFIERS: [(u8, u16); 4] =
    [(MOD_CTRL, VK_CONTROL), (MOD_SHIFT, VK_SHIFT), (MOD_ALT, VK_MENU), (MOD_WIN, VK_LWIN)];

/// Estado de uma sessão: teclas e botões que o celular deixou pressionados.
/// Tudo o que está aqui é solto quando a conexão cai.
#[derive(Default)]
pub struct KeyState {
    held_keys: Vec<u16>,
    held_buttons: Vec<u8>,
}

fn tap(out: &mut Vec<Raw>, vk: u16) {
    out.push(Raw::Key { vk, up: false });
    out.push(Raw::Key { vk, up: true });
}

impl KeyState {
    pub fn type_text(&mut self, backspaces: u16, text: &str, out: &mut Vec<Raw>) {
        for _ in 0..backspaces {
            tap(out, VK_BACK);
        }
        let mut after_cr = false;
        for unit in text.encode_utf16() {
            match unit {
                0x0D => tap(out, VK_RETURN),
                0x0A if after_cr => {}
                0x0A => tap(out, VK_RETURN),
                0x09 => tap(out, VK_TAB),
                u if u < 0x20 || u == 0x7F => {}
                u => {
                    out.push(Raw::Unicode { unit: u, up: false });
                    out.push(Raw::Unicode { unit: u, up: true });
                }
            }
            after_cr = unit == 0x0D;
        }
    }

    pub fn key(&mut self, action: u8, mods: u8, vk: u16, out: &mut Vec<Raw>) {
        if vk == 0 || vk > 0xFE {
            return;
        }
        match action {
            KEY_TAP => {
                let pressed: Vec<u16> = MODIFIERS
                    .iter()
                    .filter(|(bit, mvk)| mods & bit != 0 && *mvk != vk && !self.held_keys.contains(mvk))
                    .map(|&(_, mvk)| mvk)
                    .collect();
                for &m in &pressed {
                    out.push(Raw::Key { vk: m, up: false });
                }
                tap(out, vk);
                for &m in pressed.iter().rev() {
                    out.push(Raw::Key { vk: m, up: true });
                }
            }
            KEY_DOWN => {
                if !self.held_keys.contains(&vk) {
                    if self.held_keys.len() >= MAX_HELD {
                        return;
                    }
                    self.held_keys.push(vk);
                }
                // Repetir o "segura" é o autorrepeat normal de uma tecla física.
                out.push(Raw::Key { vk, up: false });
            }
            KEY_UP => {
                if let Some(i) = self.held_keys.iter().position(|&k| k == vk) {
                    self.held_keys.remove(i);
                    out.push(Raw::Key { vk, up: true });
                }
            }
            _ => {}
        }
    }

    pub fn char_key(
        &mut self,
        mods: u8,
        codepoint: u32,
        resolve: impl Fn(u32) -> Option<(u16, u8)>,
        out: &mut Vec<Raw>,
    ) {
        let Some(ch) = char::from_u32(codepoint) else { return };
        if ch.is_ascii_alphabetic() {
            // Com Ctrl/Alt/Win a caixa da letra não importa (o teclado do celular
            // costuma mandar maiúscula no início do campo); Shift só se pedido.
            self.key(KEY_TAP, mods, ch.to_ascii_uppercase() as u16, out);
        } else if let Some((vk, needed)) = resolve(codepoint) {
            self.key(KEY_TAP, mods | needed, vk, out);
        } else {
            let mut buf = [0u8; 4];
            self.type_text(0, ch.encode_utf8(&mut buf), out);
        }
    }

    pub fn mouse_button(&mut self, button: u8, action: u8, out: &mut Vec<Raw>) {
        if !(1..=3).contains(&button) {
            return;
        }
        match action {
            KEY_TAP => {
                out.push(Raw::MouseButton { button, up: false });
                out.push(Raw::MouseButton { button, up: true });
            }
            KEY_DOWN => {
                if !self.held_buttons.contains(&button) {
                    self.held_buttons.push(button);
                }
                out.push(Raw::MouseButton { button, up: false });
            }
            KEY_UP => {
                if let Some(i) = self.held_buttons.iter().position(|&b| b == button) {
                    self.held_buttons.remove(i);
                    out.push(Raw::MouseButton { button, up: true });
                }
            }
            _ => {}
        }
    }

    pub fn release_all(&mut self, out: &mut Vec<Raw>) {
        for vk in self.held_keys.drain(..).rev() {
            out.push(Raw::Key { vk, up: true });
        }
        for button in self.held_buttons.drain(..).rev() {
            out.push(Raw::MouseButton { button, up: true });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(out: &[Raw]) -> Vec<(u16, bool)> {
        out.iter()
            .map(|r| match *r {
                Raw::Key { vk, up } => (vk, up),
                Raw::Unicode { unit, up } => (unit, up),
                _ => panic!("esperava teclado"),
            })
            .collect()
    }

    #[test]
    fn typing_deletes_then_writes_unicode() {
        let mut s = KeyState::default();
        let mut out = vec![];
        s.type_text(2, "ã😀", &mut out);
        let mut expected = vec![
            Raw::Key { vk: VK_BACK, up: false },
            Raw::Key { vk: VK_BACK, up: true },
            Raw::Key { vk: VK_BACK, up: false },
            Raw::Key { vk: VK_BACK, up: true },
            Raw::Unicode { unit: 0xE3, up: false },
            Raw::Unicode { unit: 0xE3, up: true },
        ];
        for unit in [0xD83D, 0xDE00] {
            expected.push(Raw::Unicode { unit, up: false });
            expected.push(Raw::Unicode { unit, up: true });
        }
        assert_eq!(out, expected);
    }

    #[test]
    fn newlines_and_tabs_become_keys() {
        let mut s = KeyState::default();
        let mut out = vec![];
        s.type_text(0, "a\r\nb\n\t\u{7}", &mut out);
        let vks: Vec<u16> = out
            .iter()
            .filter_map(|r| match *r {
                Raw::Key { vk, up: false } => Some(vk),
                Raw::Unicode { unit, up: false } => Some(unit),
                _ => None,
            })
            .collect();
        assert_eq!(vks, vec!['a' as u16, VK_RETURN, 'b' as u16, VK_RETURN, VK_TAB]);
    }

    #[test]
    fn tap_wraps_modifiers_in_order() {
        let mut s = KeyState::default();
        let mut out = vec![];
        s.key(KEY_TAP, MOD_CTRL | MOD_SHIFT, 0x54, &mut out);
        assert_eq!(
            keys(&out),
            vec![
                (VK_CONTROL, false),
                (VK_SHIFT, false),
                (0x54, false),
                (0x54, true),
                (VK_SHIFT, true),
                (VK_CONTROL, true)
            ]
        );
    }

    #[test]
    fn alt_tab_keeps_alt_held_until_released() {
        let mut s = KeyState::default();
        let mut out = vec![];
        s.key(KEY_DOWN, 0, VK_MENU, &mut out);
        s.key(KEY_TAP, MOD_ALT, VK_TAB, &mut out);
        s.key(KEY_TAP, MOD_ALT, VK_TAB, &mut out);
        s.key(KEY_UP, 0, VK_MENU, &mut out);
        assert_eq!(
            keys(&out),
            vec![(VK_MENU, false), (VK_TAB, false), (VK_TAB, true), (VK_TAB, false), (VK_TAB, true), (VK_MENU, true)]
        );
    }

    #[test]
    fn release_all_frees_everything_in_reverse() {
        let mut s = KeyState::default();
        let mut out = vec![];
        s.key(KEY_DOWN, 0, VK_CONTROL, &mut out);
        s.key(KEY_DOWN, 0, VK_SHIFT, &mut out);
        s.mouse_button(1, KEY_DOWN, &mut out);
        out.clear();
        s.release_all(&mut out);
        assert_eq!(
            out,
            vec![
                Raw::Key { vk: VK_SHIFT, up: true },
                Raw::Key { vk: VK_CONTROL, up: true },
                Raw::MouseButton { button: 1, up: true }
            ]
        );
        out.clear();
        s.release_all(&mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn up_without_down_is_ignored() {
        let mut s = KeyState::default();
        let mut out = vec![];
        s.key(KEY_UP, 0, VK_MENU, &mut out);
        s.mouse_button(2, KEY_UP, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn char_key_ignores_letter_case_and_uses_layout_for_symbols() {
        let mut s = KeyState::default();
        let mut out = vec![];
        s.char_key(MOD_CTRL, 'C' as u32, |_| None, &mut out);
        assert_eq!(keys(&out), vec![(VK_CONTROL, false), (0x43, false), (0x43, true), (VK_CONTROL, true)]);

        out.clear();
        s.char_key(MOD_CTRL, '+' as u32, |_| Some((0xBB, MOD_SHIFT)), &mut out);
        assert_eq!(
            keys(&out),
            vec![
                (VK_CONTROL, false),
                (VK_SHIFT, false),
                (0xBB, false),
                (0xBB, true),
                (VK_SHIFT, true),
                (VK_CONTROL, true)
            ]
        );

        out.clear();
        s.char_key(MOD_CTRL, 'ç' as u32, |_| None, &mut out);
        assert_eq!(out, vec![Raw::Unicode { unit: 0xE7, up: false }, Raw::Unicode { unit: 0xE7, up: true }]);
    }
}
