//! Bridge between winit key events and termesh-input types.

use termesh_input::keymap::{Key, Modifiers};
use winit::event::Modifiers as WinitModifiers;
use winit::keyboard::{Key as WinitKey, NamedKey};

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

/// Convert a winit logical key to a termesh-input key.
pub fn convert_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Character(c) => {
            let s = c.as_str();
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
}
