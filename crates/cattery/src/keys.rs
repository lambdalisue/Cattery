//! Keystrokes as winit reports them, turned into what the daemon takes.
//!
//! The daemon encodes a keystroke into bytes against the terminal modes it
//! owns; a window only says which key it was, what text it composed, and
//! which modifiers were held.

use felis_client_core::{FKey, Modifiers, NamedKey};
use felis_protocol::limits::MAX_KEY_TEXT_BYTES;
use felis_protocol::messages::{InputMsg, Key, KeyEvent, KeyEventKind, KeyLocation};
use winit::event::ElementState;
use winit::keyboard::{
    Key as WinitKey, KeyLocation as WinitKeyLocation, ModifiersState, NamedKey as WinitNamedKey,
};

/// What a key event says, apart from winit's own event type, which cannot
/// be built outside winit.
pub struct Stroke<'a> {
    pub key: &'a WinitKey,
    pub text: Option<&'a str>,
    pub state: ElementState,
    pub repeat: bool,
    pub location: WinitKeyLocation,
}

impl<'a> From<&'a winit::event::KeyEvent> for Stroke<'a> {
    fn from(event: &'a winit::event::KeyEvent) -> Self {
        Self {
            key: &event.logical_key,
            text: event.text.as_deref(),
            state: event.state,
            repeat: event.repeat,
            location: event.location,
        }
    }
}

/// The input that sends `stroke`, made while `mods` are held.
pub fn input(stroke: &Stroke<'_>, mods: ModifiersState) -> InputMsg {
    // Composed text past what one keystroke may carry is an input method
    // committing a phrase, which the daemon refuses as a key.
    if let Some(text) = stroke.text.filter(|text| text.len() > MAX_KEY_TEXT_BYTES) {
        return InputMsg::KeyBytes(text.as_bytes().to_vec());
    }
    InputMsg::Key(KeyEvent {
        key: key(stroke.key),
        text: stroke.text.map(str::to_owned),
        mods: modifiers(mods),
        kind: match stroke.state {
            ElementState::Released => KeyEventKind::Release,
            ElementState::Pressed if stroke.repeat => KeyEventKind::Repeat,
            ElementState::Pressed => KeyEventKind::Press,
        },
        location: match stroke.location {
            WinitKeyLocation::Standard => KeyLocation::Standard,
            WinitKeyLocation::Left => KeyLocation::Left,
            WinitKeyLocation::Right => KeyLocation::Right,
            WinitKeyLocation::Numpad => KeyLocation::Numpad,
        },
    })
}

fn key(key: &WinitKey) -> Key {
    match key {
        WinitKey::Character(s) => Key::Character(s.to_string()),
        WinitKey::Named(named) => self::named(*named).map_or(Key::Other, Key::Named),
        _ => Key::Other,
    }
}

fn named(named: WinitNamedKey) -> Option<NamedKey> {
    Some(match named {
        WinitNamedKey::Enter => NamedKey::Enter,
        WinitNamedKey::Tab => NamedKey::Tab,
        WinitNamedKey::Escape => NamedKey::Escape,
        WinitNamedKey::Space => NamedKey::Space,
        WinitNamedKey::Backspace => NamedKey::Backspace,
        WinitNamedKey::Insert => NamedKey::Insert,
        WinitNamedKey::Delete => NamedKey::Delete,
        WinitNamedKey::Home => NamedKey::Home,
        WinitNamedKey::End => NamedKey::End,
        WinitNamedKey::PageUp => NamedKey::PageUp,
        WinitNamedKey::PageDown => NamedKey::PageDown,
        WinitNamedKey::ArrowUp => NamedKey::ArrowUp,
        WinitNamedKey::ArrowDown => NamedKey::ArrowDown,
        WinitNamedKey::ArrowLeft => NamedKey::ArrowLeft,
        WinitNamedKey::ArrowRight => NamedKey::ArrowRight,
        other => NamedKey::F(function_key(other)?),
    })
}

fn function_key(named: WinitNamedKey) -> Option<FKey> {
    use WinitNamedKey::*;
    const KEYS: [WinitNamedKey; 35] = [
        F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20,
        F21, F22, F23, F24, F25, F26, F27, F28, F29, F30, F31, F32, F33, F34, F35,
    ];
    let index = KEYS.iter().position(|key| *key == named)?;
    FKey::new(u8::try_from(index + 1).ok()?)
}

fn modifiers(mods: ModifiersState) -> Modifiers {
    let mut out = Modifiers::empty();
    out.set(Modifiers::CONTROL, mods.control_key());
    out.set(Modifiers::SHIFT, mods.shift_key());
    out.set(Modifiers::ALT, mods.alt_key());
    out.set(Modifiers::SUPER, mods.super_key());
    out
}

/// Whether `key` with `mods` held is the platform's chord for an app's
/// settings: Command-comma on macOS, Control-comma elsewhere.
pub fn opens_preferences(key: &WinitKey, mods: ModifiersState) -> bool {
    let primary = if cfg!(target_os = "macos") {
        ModifiersState::SUPER
    } else {
        ModifiersState::CONTROL
    };
    mods == primary && matches!(key, WinitKey::Character(c) if c == ",")
}

#[cfg(test)]
mod tests {
    use winit::keyboard::SmolStr;

    use super::*;

    fn press(key: WinitKey, text: Option<&str>) -> KeyEvent {
        let stroke = Stroke {
            key: &key,
            text,
            state: ElementState::Pressed,
            repeat: false,
            location: WinitKeyLocation::Standard,
        };
        key_event(input(&stroke, ModifiersState::empty()))
    }

    fn key_event(input: InputMsg) -> KeyEvent {
        match input {
            InputMsg::Key(event) => event,
            other => panic!("not a key event: {other:?}"),
        }
    }

    fn character(s: &str) -> WinitKey {
        WinitKey::Character(SmolStr::new(s))
    }

    fn primary() -> ModifiersState {
        if cfg!(target_os = "macos") {
            ModifiersState::SUPER
        } else {
            ModifiersState::CONTROL
        }
    }

    #[test]
    fn the_platform_s_settings_chord_opens_preferences() {
        assert!(opens_preferences(&character(","), primary()));
    }

    #[test]
    fn a_comma_alone_or_with_more_held_is_typed() {
        assert!(!opens_preferences(&character(","), ModifiersState::empty()));
        assert!(!opens_preferences(
            &character(","),
            primary() | ModifiersState::SHIFT
        ));
        assert!(!opens_preferences(&character(","), ModifiersState::ALT));
    }

    #[test]
    fn the_chord_on_another_key_is_typed() {
        assert!(!opens_preferences(&character("."), primary()));
    }

    #[test]
    fn a_letter_is_its_character_with_the_text_it_composed() {
        let event = press(character("a"), Some("a"));
        assert_eq!(event.key, Key::Character("a".to_owned()));
        assert_eq!(event.text.as_deref(), Some("a"));
        assert_eq!(event.kind, KeyEventKind::Press);
    }

    #[test]
    fn enter_is_a_named_key() {
        let event = press(WinitKey::Named(WinitNamedKey::Enter), Some("\r"));
        assert_eq!(event.key, Key::Named(NamedKey::Enter));
    }

    #[test]
    fn arrows_and_editing_keys_are_named() {
        for (winit, felis) in [
            (WinitNamedKey::ArrowUp, NamedKey::ArrowUp),
            (WinitNamedKey::Backspace, NamedKey::Backspace),
            (WinitNamedKey::Escape, NamedKey::Escape),
            (WinitNamedKey::PageDown, NamedKey::PageDown),
        ] {
            assert_eq!(press(WinitKey::Named(winit), None).key, Key::Named(felis));
        }
    }

    #[test]
    fn function_keys_are_named_by_their_number() {
        let first = press(WinitKey::Named(WinitNamedKey::F1), None);
        let last = press(WinitKey::Named(WinitNamedKey::F35), None);
        assert_eq!(first.key, Key::Named(NamedKey::F(FKey::new(1).unwrap())));
        assert_eq!(last.key, Key::Named(NamedKey::F(FKey::new(35).unwrap())));
    }

    #[test]
    fn a_modifier_alone_has_no_encoding_of_its_own() {
        let event = press(WinitKey::Named(WinitNamedKey::Shift), None);
        assert_eq!(event.key, Key::Other);
    }

    #[test]
    fn a_key_felis_does_not_name_passes_its_text_through() {
        let event = press(WinitKey::Named(WinitNamedKey::MediaPlayPause), None);
        assert_eq!(event.key, Key::Other);
    }

    #[test]
    fn the_modifiers_held_are_carried() {
        let key = character("c");
        let stroke = Stroke {
            key: &key,
            text: None,
            state: ElementState::Pressed,
            repeat: false,
            location: WinitKeyLocation::Standard,
        };
        let event = key_event(input(
            &stroke,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        ));
        assert_eq!(event.mods, Modifiers::CONTROL | Modifiers::SHIFT);
    }

    #[test]
    fn a_held_key_repeats_and_a_lifted_key_is_released() {
        let key = character("x");
        let mut stroke = Stroke {
            key: &key,
            text: Some("x"),
            state: ElementState::Pressed,
            repeat: true,
            location: WinitKeyLocation::Standard,
        };
        let repeated = key_event(input(&stroke, ModifiersState::empty()));
        assert_eq!(repeated.kind, KeyEventKind::Repeat);
        stroke.state = ElementState::Released;
        stroke.repeat = false;
        let released = key_event(input(&stroke, ModifiersState::empty()));
        assert_eq!(released.kind, KeyEventKind::Release);
    }

    #[test]
    fn the_keypad_is_told_apart() {
        let key = character("1");
        let stroke = Stroke {
            key: &key,
            text: Some("1"),
            state: ElementState::Pressed,
            repeat: false,
            location: WinitKeyLocation::Numpad,
        };
        let event = key_event(input(&stroke, ModifiersState::empty()));
        assert_eq!(event.location, KeyLocation::Numpad);
    }

    #[test]
    fn text_longer_than_a_keystroke_carries_is_sent_as_it_is() {
        let text = "x".repeat(MAX_KEY_TEXT_BYTES + 1);
        let key = character("x");
        let stroke = Stroke {
            key: &key,
            text: Some(&text),
            state: ElementState::Pressed,
            repeat: false,
            location: WinitKeyLocation::Standard,
        };
        assert_eq!(
            input(&stroke, ModifiersState::empty()),
            InputMsg::KeyBytes(text.into_bytes()),
        );
    }
}
