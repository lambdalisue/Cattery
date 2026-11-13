//! Keystrokes as winit reports them, turned into what the daemon takes.
//!
//! The daemon encodes a keystroke into bytes against the terminal modes it
//! owns; a window only says which key it was, what text it composed, and
//! which modifiers were held.

use felis_client_core::{FKey, Modifiers, NamedKey};
use felis_protocol::limits::MAX_KEY_TEXT_BYTES;
use felis_protocol::messages::{InputMsg, Key, KeyEvent, KeyEventKind, KeyLocation};
use felis_render_core::overlay::PreeditOverlay;

use crate::settings::Zoom;
use winit::event::{ElementState, Ime};
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

/// What the app does, rather than the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// The Preferences window's page about Cattery.
    About,
    Preferences,
    /// A new window on a session in the home directory.
    NewWindow,
    /// A new window on a session in the directory of the window it is
    /// opened from.
    NewWindowHere,
    CloseWindow,
    /// End every window's session and the app.
    Quit,
    Zoom(Zoom),
}

/// The command `key` with `mods` held is the platform's chord for.
///
/// On macOS the chords hold Command, which a terminal program never sees.
/// Elsewhere Control-letter is a terminal's own, so the window chords add
/// Shift, as other terminals there do.
pub fn command(key: &WinitKey, mods: ModifiersState) -> Option<Command> {
    const CHORDS: &[(&str, ModifiersState, Command)] = if cfg!(target_os = "macos") {
        &[
            (",", ModifiersState::SUPER, Command::Preferences),
            ("n", ModifiersState::SUPER, Command::NewWindow),
            (
                "n",
                ModifiersState::SUPER.union(ModifiersState::SHIFT),
                Command::NewWindowHere,
            ),
            ("w", ModifiersState::SUPER, Command::CloseWindow),
        ]
    } else {
        &[
            (",", ModifiersState::CONTROL, Command::Preferences),
            (
                "n",
                ModifiersState::CONTROL.union(ModifiersState::SHIFT),
                Command::NewWindow,
            ),
            (
                "n",
                ModifiersState::CONTROL
                    .union(ModifiersState::SHIFT)
                    .union(ModifiersState::ALT),
                Command::NewWindowHere,
            ),
            (
                "w",
                ModifiersState::CONTROL.union(ModifiersState::SHIFT),
                Command::CloseWindow,
            ),
        ]
    };
    let WinitKey::Character(typed) = key else {
        return None;
    };
    // Shift turns the letter upper case.
    let typed = typed.to_lowercase();
    // The zoom keys are spelled with Shift on some layouts and without it
    // on others (US `+` is Shift-`=`), so both are taken.
    let zooms = if cfg!(target_os = "macos") {
        [
            ModifiersState::SUPER,
            ModifiersState::SUPER.union(ModifiersState::SHIFT),
        ]
    } else {
        [ModifiersState::CONTROL.union(ModifiersState::SHIFT); 2]
    };
    let zoom = match typed.as_str() {
        "=" | "+" => Some(Zoom::In),
        "-" | "_" => Some(Zoom::Out),
        "0" | ")" => Some(Zoom::Reset),
        _ => None,
    };
    if let Some(zoom) = zoom.filter(|_| zooms.contains(&mods)) {
        return Some(Command::Zoom(zoom));
    }
    CHORDS
        .iter()
        .find(|(key, chord, _)| *key == typed && *chord == mods)
        .map(|(_, _, command)| *command)
}

/// The input an input method's event sends: its committed text, if any.
pub fn committed(ime: &Ime) -> Option<InputMsg> {
    // Committed text is final: no modifier applies to it, so it reaches
    // the program as its bytes, as felis's own window sends it.
    match ime {
        Ime::Commit(text) if !text.is_empty() => Some(InputMsg::KeyBytes(text.as_bytes().to_vec())),
        _ => None,
    }
}

/// The text being composed, to draw at the cursor at `at`; `None` once
/// nothing is being composed.
pub fn composing(ime: &Ime, at: (u16, u16)) -> Option<PreeditOverlay> {
    match ime {
        Ime::Preedit(text, cursor) if !text.is_empty() => Some(PreeditOverlay {
            anchor: at,
            text: text.clone(),
            cursor: *cursor,
        }),
        _ => None,
    }
}

/// What a keystroke sends in place of its key when an input method
/// inserted `inserted` for it without composing it first; `None` when the
/// key itself is to be sent, as when the inserted text is the key's own.
pub fn inserted_instead(inserted: Option<&str>, typed: Option<&str>) -> Option<InputMsg> {
    let inserted = inserted.filter(|text| {
        !text.is_empty() && !text.chars().any(char::is_control) && Some(*text) != typed
    })?;
    Some(InputMsg::KeyBytes(inserted.as_bytes().to_vec()))
}

/// Which way Shift with Page Up or Page Down scrolls history by half a
/// screen: up into it being positive.
pub fn page(key: &WinitKey, mods: ModifiersState) -> Option<i32> {
    if mods != ModifiersState::SHIFT {
        return None;
    }
    match key {
        WinitKey::Named(WinitNamedKey::PageUp) => Some(1),
        WinitKey::Named(WinitNamedKey::PageDown) => Some(-1),
        _ => None,
    }
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

    const SUPER: ModifiersState = ModifiersState::SUPER;
    const CONTROL: ModifiersState = ModifiersState::CONTROL;
    const SHIFT: ModifiersState = ModifiersState::SHIFT;
    const ALT: ModifiersState = ModifiersState::ALT;

    #[cfg(target_os = "macos")]
    #[test]
    fn the_app_s_chords_on_macos_hold_command() {
        for (key, mods, command) in [
            (",", SUPER, Command::Preferences),
            ("n", SUPER, Command::NewWindow),
            ("N", SUPER.union(SHIFT), Command::NewWindowHere),
            ("w", SUPER, Command::CloseWindow),
        ] {
            assert_eq!(
                super::command(&character(key), mods),
                Some(command),
                "{key}"
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn the_app_s_chords_elsewhere_hold_control_and_shift_for_windows() {
        for (key, mods, command) in [
            (",", CONTROL, Command::Preferences),
            ("N", CONTROL.union(SHIFT), Command::NewWindow),
            ("N", CONTROL.union(SHIFT).union(ALT), Command::NewWindowHere),
            ("W", CONTROL.union(SHIFT), Command::CloseWindow),
        ] {
            assert_eq!(
                super::command(&character(key), mods),
                Some(command),
                "{key}"
            );
        }
    }

    #[test]
    fn zoom_chords_take_either_spelling_of_the_key() {
        let mods = if cfg!(target_os = "macos") {
            SUPER
        } else {
            CONTROL.union(SHIFT)
        };
        for (key, zoom) in [
            ("=", Zoom::In),
            ("+", Zoom::In),
            ("-", Zoom::Out),
            ("_", Zoom::Out),
            ("0", Zoom::Reset),
        ] {
            assert_eq!(
                command(&character(key), mods),
                Some(Command::Zoom(zoom)),
                "{key}"
            );
        }
    }

    #[test]
    fn shift_with_the_page_keys_pages_through_history() {
        let up = WinitKey::Named(WinitNamedKey::PageUp);
        let down = WinitKey::Named(WinitNamedKey::PageDown);
        assert_eq!(page(&up, SHIFT), Some(1));
        assert_eq!(page(&down, SHIFT), Some(-1));
        assert_eq!(page(&up, ModifiersState::empty()), None);
        assert_eq!(page(&character("a"), SHIFT), None);
    }

    #[test]
    fn a_key_typed_alone_or_with_control_alone_reaches_the_session() {
        for key in [",", "n", "w"] {
            assert_eq!(command(&character(key), ModifiersState::empty()), None);
        }
        if !cfg!(target_os = "macos") {
            assert_eq!(command(&character("n"), CONTROL), None);
            assert_eq!(command(&character("w"), CONTROL), None);
        }
    }

    #[test]
    fn other_keys_with_the_chord_s_modifiers_reach_the_session() {
        let mods = if cfg!(target_os = "macos") {
            SUPER
        } else {
            CONTROL
        };
        assert_eq!(command(&character("."), mods), None);
        assert_eq!(command(&character(","), mods.union(ALT)), None);
    }

    #[test]
    fn committed_text_is_sent_as_it_is() {
        let ime = Ime::Commit("日本語".to_owned());
        assert_eq!(
            committed(&ime),
            Some(InputMsg::KeyBytes("日本語".as_bytes().to_vec()))
        );
    }

    #[test]
    fn an_empty_commit_sends_nothing() {
        // Some input methods commit nothing when composing is cancelled.
        assert_eq!(committed(&Ime::Commit(String::new())), None);
    }

    #[test]
    fn composing_sends_nothing() {
        assert_eq!(
            committed(&Ime::Preedit("にほ".to_owned(), Some((0, 6)))),
            None
        );
    }

    #[test]
    fn text_being_composed_is_drawn_at_the_cursor() {
        let overlay = composing(&Ime::Preedit("にほ".to_owned(), Some((3, 6))), (4, 10)).unwrap();
        assert_eq!(overlay.anchor, (4, 10));
        assert_eq!(overlay.text, "にほ");
        assert_eq!(overlay.cursor, Some((3, 6)));
    }

    #[test]
    fn nothing_composed_draws_nothing() {
        for ime in [
            Ime::Preedit(String::new(), None),
            Ime::Commit("日本".to_owned()),
            Ime::Enabled,
            Ime::Disabled,
        ] {
            assert!(composing(&ime, (0, 0)).is_none(), "{ime:?}");
        }
    }

    #[test]
    fn text_an_input_method_inserts_for_a_key_is_sent_instead_of_it() {
        assert_eq!(
            inserted_instead(Some("あ"), Some("a")),
            Some(InputMsg::KeyBytes("あ".as_bytes().to_vec())),
        );
    }

    #[test]
    fn a_key_inserting_its_own_character_is_sent_as_a_key() {
        assert_eq!(inserted_instead(Some("a"), Some("a")), None);
    }

    #[test]
    fn a_key_nothing_was_inserted_for_is_sent_as_a_key() {
        assert_eq!(inserted_instead(None, Some("a")), None);
    }

    #[test]
    fn inserted_control_characters_are_left_to_the_key() {
        assert_eq!(inserted_instead(Some("\r"), Some("\r")), None);
        assert_eq!(inserted_instead(Some("\t"), None), None);
        assert_eq!(inserted_instead(Some(""), Some("a")), None);
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
