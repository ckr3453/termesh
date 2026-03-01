//! Bridge between winit key events and termesh-input types.

use termesh_input::keymap::{Key, Modifiers};
use winit::event::Modifiers as WinitModifiers;
use winit::keyboard::{Key as WinitKey, KeyCode, NamedKey, PhysicalKey};

/// Convert winit modifiers to termesh-input modifiers.
pub fn convert_modifiers(mods: &WinitModifiers) -> Modifiers {
    let state = mods.state();
    Modifiers {
        ctrl: state.control_key(),
        alt: state.alt_key(),
        shift: state.shift_key(),
        logo: state.super_key(),
    }
}

/// Convert a winit physical key to a termesh-input key (fallback).
///
/// Used when `convert_key` returns None for the logical key, ensuring
/// modifier+key combos (e.g. Ctrl+H on Windows produces `\x08` instead of `h`)
/// are still recognized for keybinding lookup.
pub fn convert_physical_key(physical: &PhysicalKey) -> Option<Key> {
    match physical {
        PhysicalKey::Code(code) => match code {
            KeyCode::Enter | KeyCode::NumpadEnter => Some(Key::Enter),
            KeyCode::Escape => Some(Key::Escape),
            KeyCode::Tab => Some(Key::Tab),
            KeyCode::Backspace => Some(Key::Backspace),
            KeyCode::Delete => Some(Key::Delete),
            KeyCode::ArrowLeft => Some(Key::Left),
            KeyCode::ArrowRight => Some(Key::Right),
            KeyCode::ArrowUp => Some(Key::Up),
            KeyCode::ArrowDown => Some(Key::Down),
            // Letters (Ctrl+letter produces control chars, physical key recovers letter)
            KeyCode::KeyA => Some(Key::Char('a')),
            KeyCode::KeyB => Some(Key::Char('b')),
            KeyCode::KeyC => Some(Key::Char('c')),
            KeyCode::KeyD => Some(Key::Char('d')),
            KeyCode::KeyE => Some(Key::Char('e')),
            KeyCode::KeyF => Some(Key::Char('f')),
            KeyCode::KeyG => Some(Key::Char('g')),
            KeyCode::KeyH => Some(Key::Char('h')),
            KeyCode::KeyI => Some(Key::Char('i')),
            KeyCode::KeyJ => Some(Key::Char('j')),
            KeyCode::KeyK => Some(Key::Char('k')),
            KeyCode::KeyL => Some(Key::Char('l')),
            KeyCode::KeyM => Some(Key::Char('m')),
            KeyCode::KeyN => Some(Key::Char('n')),
            KeyCode::KeyO => Some(Key::Char('o')),
            KeyCode::KeyP => Some(Key::Char('p')),
            KeyCode::KeyQ => Some(Key::Char('q')),
            KeyCode::KeyR => Some(Key::Char('r')),
            KeyCode::KeyS => Some(Key::Char('s')),
            KeyCode::KeyT => Some(Key::Char('t')),
            KeyCode::KeyU => Some(Key::Char('u')),
            KeyCode::KeyV => Some(Key::Char('v')),
            KeyCode::KeyW => Some(Key::Char('w')),
            KeyCode::KeyX => Some(Key::Char('x')),
            KeyCode::KeyY => Some(Key::Char('y')),
            KeyCode::KeyZ => Some(Key::Char('z')),
            // Digits (Ctrl+number may not produce the digit as logical key)
            KeyCode::Digit1 => Some(Key::Char('1')),
            KeyCode::Digit2 => Some(Key::Char('2')),
            KeyCode::Digit3 => Some(Key::Char('3')),
            KeyCode::Digit4 => Some(Key::Char('4')),
            // Symbols that conflict with Ctrl (Ctrl+[ = ESC, Ctrl+] = 0x1d)
            KeyCode::BracketLeft => Some(Key::Char('[')),
            KeyCode::BracketRight => Some(Key::Char(']')),
            _ => None,
        },
        _ => None,
    }
}

/// Convert a winit logical key to a termesh-input key.
pub fn convert_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Character(c) => {
            let s = c.as_str();
            // Ctrl+Enter may produce "\n" or "\r" as a Character on some platforms
            if s == "\n" || s == "\r" {
                return Some(Key::Enter);
            }
            if s.len() == 1 {
                Some(Key::Char(s.chars().next().unwrap().to_ascii_lowercase()))
            } else {
                None
            }
        }
        WinitKey::Named(named) => match named {
            NamedKey::Enter => Some(Key::Enter),
            NamedKey::Escape => Some(Key::Escape),
            NamedKey::Tab => Some(Key::Tab),
            NamedKey::Backspace => Some(Key::Backspace),
            NamedKey::Delete => Some(Key::Delete),
            NamedKey::ArrowLeft => Some(Key::Left),
            NamedKey::ArrowRight => Some(Key::Right),
            NamedKey::ArrowUp => Some(Key::Up),
            NamedKey::ArrowDown => Some(Key::Down),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_key_char() {
        let key = WinitKey::Character("a".into());
        assert_eq!(convert_key(&key), Some(Key::Char('a')));
    }

    #[test]
    fn test_convert_key_enter() {
        let key = WinitKey::Named(NamedKey::Enter);
        assert_eq!(convert_key(&key), Some(Key::Enter));
    }

    #[test]
    fn test_convert_key_arrow() {
        assert_eq!(
            convert_key(&WinitKey::Named(NamedKey::ArrowLeft)),
            Some(Key::Left)
        );
        assert_eq!(
            convert_key(&WinitKey::Named(NamedKey::ArrowDown)),
            Some(Key::Down)
        );
    }

    #[test]
    fn test_convert_key_multi_char_returns_none() {
        let key = WinitKey::Character("abc".into());
        assert_eq!(convert_key(&key), None);
    }

    #[test]
    fn test_convert_key_unknown_named() {
        let key = WinitKey::Named(NamedKey::F1);
        assert_eq!(convert_key(&key), None);
    }

    #[test]
    fn test_convert_key_newline_as_enter() {
        // Ctrl+Enter may produce "\n" as Character on Windows
        let key = WinitKey::Character("\n".into());
        assert_eq!(convert_key(&key), Some(Key::Enter));
    }

    #[test]
    fn test_convert_key_cr_as_enter() {
        // Ctrl+Enter may produce "\r" as Character
        let key = WinitKey::Character("\r".into());
        assert_eq!(convert_key(&key), Some(Key::Enter));
    }

    #[test]
    fn test_convert_physical_enter() {
        assert_eq!(
            convert_physical_key(&PhysicalKey::Code(KeyCode::Enter)),
            Some(Key::Enter)
        );
    }

    #[test]
    fn test_convert_physical_numpad_enter() {
        assert_eq!(
            convert_physical_key(&PhysicalKey::Code(KeyCode::NumpadEnter)),
            Some(Key::Enter)
        );
    }

    #[test]
    fn test_convert_physical_arrows() {
        assert_eq!(
            convert_physical_key(&PhysicalKey::Code(KeyCode::ArrowUp)),
            Some(Key::Up)
        );
        assert_eq!(
            convert_physical_key(&PhysicalKey::Code(KeyCode::ArrowDown)),
            Some(Key::Down)
        );
    }
}
