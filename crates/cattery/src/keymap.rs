//! Which keys run which of Cattery's commands: the platform's defaults, with
//! a user's `keys.toml` over them.

use std::fmt;
use std::path::{Path, PathBuf};

use felis_protocol::messages::PromptJump;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::keys::Command;
use crate::settings::Zoom;

/// The keys Cattery binds before a user's own.
const DEFAULTS: &str = if cfg!(target_os = "macos") {
    include_str!("../keys/macos.toml")
} else {
    include_str!("../keys/other.toml")
};

/// What a key is bound to that takes its default away.
const UNBOUND: &str = "none";

/// The commands a key can be bound to, by the names the file gives them.
const NAMES: &[(&str, Command)] = &[
    ("app.about", Command::About),
    ("app.preferences", Command::Preferences),
    ("app.reload", Command::Reload),
    ("app.restart_daemon", Command::RestartDaemon),
    ("app.help", Command::Help),
    ("app.report_issue", Command::ReportIssue),
    ("app.quit", Command::Quit),
    ("window.new", Command::NewWindow),
    ("window.new_here", Command::NewWindowHere),
    ("window.close", Command::CloseWindow),
    ("window.close_all", Command::CloseAllWindows),
    ("session.next", Command::NextSession),
    ("session.previous", Command::PreviousSession),
    ("session.end", Command::EndSession),
    ("palette.open", Command::Palette),
    ("search.find", Command::Find),
    ("edit.copy", Command::Copy),
    ("edit.paste", Command::Paste),
    ("view.zoom_in", Command::Zoom(Zoom::In)),
    ("view.zoom_out", Command::Zoom(Zoom::Out)),
    ("view.zoom_reset", Command::Zoom(Zoom::Reset)),
    ("prompt.previous", Command::Prompt(PromptJump::Previous)),
    ("prompt.next", Command::Prompt(PromptJump::Next)),
];

/// The command a name stands for.
pub fn command(name: &str) -> Option<Command> {
    NAMES
        .iter()
        .find(|(each, _)| *each == name)
        .map(|(_, command)| *command)
}

/// The keys named for the keys a chord can hold, and the key each is.
const NAMED: &[(&str, NamedKey)] = &[
    ("Enter", NamedKey::Enter),
    ("Escape", NamedKey::Escape),
    ("Tab", NamedKey::Tab),
    ("Backspace", NamedKey::Backspace),
    ("Delete", NamedKey::Delete),
    ("Insert", NamedKey::Insert),
    ("Home", NamedKey::Home),
    ("End", NamedKey::End),
    ("PageUp", NamedKey::PageUp),
    ("PageDown", NamedKey::PageDown),
    ("ArrowUp", NamedKey::ArrowUp),
    ("ArrowDown", NamedKey::ArrowDown),
    ("ArrowLeft", NamedKey::ArrowLeft),
    ("ArrowRight", NamedKey::ArrowRight),
    ("F1", NamedKey::F1),
    ("F2", NamedKey::F2),
    ("F3", NamedKey::F3),
    ("F4", NamedKey::F4),
    ("F5", NamedKey::F5),
    ("F6", NamedKey::F6),
    ("F7", NamedKey::F7),
    ("F8", NamedKey::F8),
    ("F9", NamedKey::F9),
    ("F10", NamedKey::F10),
    ("F11", NamedKey::F11),
    ("F12", NamedKey::F12),
];

/// The key of a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordKey {
    /// A character, a letter in lower case.
    Char(char),
    Named(NamedKey),
}

/// One key with the modifiers held for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub mods: ModifiersState,
    pub key: ChordKey,
}

impl Chord {
    /// The chord `text` writes: modifiers and a key joined by `+`, as
    /// `Mod+Shift+n`. `Mod` is Command on macOS and Control with Shift
    /// elsewhere, where Control with a letter is the terminal's own.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (mods_text, key_text) = text.rsplit_once('+').unwrap_or(("", text));
        if key_text.is_empty() {
            return Err(format!("{text:?} names no key"));
        }
        let mut mods = ModifiersState::empty();
        for name in mods_text.split('+').filter(|_| !mods_text.is_empty()) {
            let one = match name {
                "Mod" if cfg!(target_os = "macos") => ModifiersState::SUPER,
                "Mod" => ModifiersState::CONTROL | ModifiersState::SHIFT,
                "Cmd" => ModifiersState::SUPER,
                "Ctrl" => ModifiersState::CONTROL,
                "Alt" => ModifiersState::ALT,
                "Shift" => ModifiersState::SHIFT,
                _ => return Err(format!("{name:?} in {text:?} is no modifier")),
            };
            if mods.intersects(one) && name != "Shift" {
                return Err(format!("{name:?} is held twice in {text:?}"));
            }
            mods |= one;
        }
        let key = if let Some((_, named)) = NAMED.iter().find(|(name, _)| *name == key_text) {
            ChordKey::Named(*named)
        } else if key_text == "Space" {
            ChordKey::Char(' ')
        } else if key_text == "Plus" {
            ChordKey::Char('+')
        } else {
            let mut chars = key_text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_uppercase() => {
                    mods |= ModifiersState::SHIFT;
                    ChordKey::Char(c.to_lowercase().next().unwrap_or(c))
                }
                (Some(c), None) => ChordKey::Char(c),
                _ => return Err(format!("{key_text:?} in {text:?} is no key")),
            }
        };
        Ok(Self { mods, key })
    }

    /// The chord `key` pressed with `mods` held is, where it is one a
    /// binding can name.
    pub fn pressed(key: &Key, mods: ModifiersState) -> Option<Self> {
        let mods = mods
            & (ModifiersState::SUPER
                | ModifiersState::CONTROL
                | ModifiersState::ALT
                | ModifiersState::SHIFT);
        let key = match key {
            Key::Named(NamedKey::Space) => ChordKey::Char(' '),
            Key::Named(named) => ChordKey::Named(*named),
            Key::Character(typed) => {
                let mut chars = typed.chars();
                let (Some(c), None) = (chars.next(), chars.next()) else {
                    return None;
                };
                ChordKey::Char(c.to_lowercase().next().unwrap_or(c))
            }
            _ => return None,
        };
        Some(Self { mods, key })
    }

    /// Whether `pressed` is this chord. A character other than a letter is
    /// the one the layout types, Shift included, so Shift is asked of it
    /// only where the chord says so.
    pub fn matches(&self, pressed: &Self) -> bool {
        if self.key != pressed.key {
            return false;
        }
        match self.key {
            ChordKey::Char(c) if !c.is_alphabetic() && !self.mods.shift_key() => {
                self.mods == pressed.mods - ModifiersState::SHIFT
            }
            _ => self.mods == pressed.mods,
        }
    }
}

impl fmt::Display for Chord {
    /// The chord as the platform writes chords: `⇧⌘N` on macOS,
    /// `Ctrl+Shift+N` elsewhere.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = match self.key {
            ChordKey::Char(' ') => "Space".to_owned(),
            ChordKey::Char(c) => c.to_uppercase().to_string(),
            ChordKey::Named(named) => match named {
                NamedKey::ArrowUp if cfg!(target_os = "macos") => "↑".to_owned(),
                NamedKey::ArrowDown if cfg!(target_os = "macos") => "↓".to_owned(),
                NamedKey::ArrowLeft if cfg!(target_os = "macos") => "←".to_owned(),
                NamedKey::ArrowRight if cfg!(target_os = "macos") => "→".to_owned(),
                _ => NAMED
                    .iter()
                    .find(|(_, each)| *each == named)
                    .map_or_else(|| format!("{named:?}"), |(name, _)| (*name).to_owned()),
            },
        };
        let held = [
            (ModifiersState::CONTROL, "⌃", "Ctrl"),
            (ModifiersState::ALT, "⌥", "Alt"),
            (ModifiersState::SHIFT, "⇧", "Shift"),
            (ModifiersState::SUPER, "⌘", "Super"),
        ]
        .into_iter()
        .filter(|(mod_, ..)| self.mods.contains(*mod_));
        if cfg!(target_os = "macos") {
            for (_, symbol, _) in held {
                f.write_str(symbol)?;
            }
            f.write_str(&key)
        } else {
            for (_, _, name) in held {
                write!(f, "{name}+")?;
            }
            f.write_str(&key)
        }
    }
}

/// An entry of a keys file that is not taken, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub key: String,
    pub reason: String,
}

/// The keys bound to Cattery's commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    bindings: Vec<(Chord, Command)>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut keymap = Self {
            bindings: Vec::new(),
        };
        // The defaults are Cattery's own and read cleanly; a test holds it.
        let _ = keymap.overlay(DEFAULTS);
        keymap
    }
}

impl Keymap {
    /// The defaults with the keys file `text` over them, and the entries of
    /// it that were not taken.
    pub fn with_user(text: &str) -> (Self, Vec<Problem>) {
        let mut keymap = Self::default();
        let problems = keymap.overlay(text);
        (keymap, problems)
    }

    /// Bind the keys of the file `text`, each in place of what its chord
    /// was bound to; an entry that does not read is left out and told.
    fn overlay(&mut self, text: &str) -> Vec<Problem> {
        let table = match text.parse::<toml_edit::DocumentMut>() {
            Ok(doc) => doc,
            Err(err) => {
                return vec![Problem {
                    key: String::new(),
                    reason: err.to_string(),
                }];
            }
        };
        let mut problems = Vec::new();
        for (key, item) in table.iter() {
            let problem = |reason: String| Problem {
                key: key.to_owned(),
                reason,
            };
            let chord = match Chord::parse(key) {
                Ok(chord) => chord,
                Err(reason) => {
                    problems.push(problem(reason));
                    continue;
                }
            };
            let Some(name) = item.as_str() else {
                problems.push(problem("is not a command's name".to_owned()));
                continue;
            };
            self.bindings.retain(|(bound, _)| *bound != chord);
            if name == UNBOUND {
                continue;
            }
            match command(name) {
                Some(command) => self.bindings.push((chord, command)),
                None => problems.push(problem(format!("{name:?} is no command"))),
            }
        }
        problems
    }

    /// The command `key` with `mods` held runs, if it is bound.
    pub fn command(&self, key: &Key, mods: ModifiersState) -> Option<Command> {
        let pressed = Chord::pressed(key, mods)?;
        self.bindings
            .iter()
            .find(|(chord, _)| chord.matches(&pressed))
            .map(|(_, command)| *command)
    }

    /// The first chord bound to `command`, to show beside it.
    pub fn chord(&self, command: Command) -> Option<Chord> {
        self.bindings
            .iter()
            .find(|(_, bound)| *bound == command)
            .map(|(chord, _)| *chord)
    }
}

/// The keys file beside the settings file at `settings`.
pub fn path(settings: &Path) -> PathBuf {
    settings.with_file_name("keys.toml")
}

/// The keys in the file at `path`, over the defaults; the defaults alone
/// where there is no file.
pub fn load(path: &Path) -> (Keymap, Vec<Problem>) {
    match std::fs::read_to_string(path) {
        Ok(text) => Keymap::with_user(&text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (Keymap::default(), Vec::new()),
        Err(err) => (
            Keymap::default(),
            vec![Problem {
                key: String::new(),
                reason: format!("{}: {err}", path.display()),
            }],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: ModifiersState = ModifiersState::CONTROL;
    const SHIFT: ModifiersState = ModifiersState::SHIFT;
    const ALT: ModifiersState = ModifiersState::ALT;
    const CMD: ModifiersState = ModifiersState::SUPER;

    fn typed(text: &str) -> Key {
        Key::Character(text.into())
    }

    #[test]
    fn the_defaults_read_cleanly_and_name_every_key_once() {
        let mut keymap = Keymap {
            bindings: Vec::new(),
        };
        assert_eq!(keymap.overlay(DEFAULTS), []);
        let chords: Vec<Chord> = keymap.bindings.iter().map(|(chord, _)| *chord).collect();
        for (at, chord) in chords.iter().enumerate() {
            assert!(!chords[..at].contains(chord), "{chord} is bound twice");
        }
    }

    #[test]
    fn every_command_with_a_name_reads_back_by_it() {
        for (name, each) in NAMES {
            assert_eq!(command(name), Some(*each), "{name}");
        }
    }

    #[test]
    fn mod_is_command_on_macos_and_control_with_shift_elsewhere() {
        let expected = if cfg!(target_os = "macos") {
            CMD
        } else {
            CTRL | SHIFT
        };
        assert_eq!(Chord::parse("Mod+n").unwrap().mods, expected);
    }

    #[test]
    fn a_capital_letter_holds_shift() {
        assert_eq!(Chord::parse("Ctrl+N"), Chord::parse("Ctrl+Shift+n"));
        assert_eq!(
            Chord::parse("Ctrl+N").unwrap(),
            Chord {
                mods: CTRL | SHIFT,
                key: ChordKey::Char('n')
            }
        );
    }

    #[test]
    fn space_and_plus_have_names_of_their_own() {
        assert_eq!(Chord::parse("Ctrl+Space").unwrap().key, ChordKey::Char(' '));
        assert_eq!(Chord::parse("Cmd+Plus").unwrap().key, ChordKey::Char('+'));
    }

    #[test]
    fn a_named_key_is_written_as_winit_names_it() {
        assert_eq!(
            Chord::parse("Ctrl+Shift+PageDown").unwrap(),
            Chord {
                mods: CTRL | SHIFT,
                key: ChordKey::Named(NamedKey::PageDown)
            }
        );
    }

    #[test]
    fn what_is_not_a_chord_is_refused() {
        for text in ["", "Ctrl+", "Hyper+a", "Ctrl+Ctrl+a", "Ctrl+Pagedown"] {
            assert!(Chord::parse(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn a_letter_matches_with_exactly_the_modifiers_written() {
        let chord = Chord::parse("Ctrl+Shift+n").unwrap();
        let pressed = |key: &str, mods| Chord::pressed(&typed(key), mods).unwrap();
        assert!(chord.matches(&pressed("N", CTRL | SHIFT)));
        assert!(chord.matches(&pressed("n", CTRL | SHIFT)));
        assert!(!chord.matches(&pressed("n", CTRL)));
        assert!(!chord.matches(&pressed("n", CTRL | SHIFT | ALT)));
    }

    #[test]
    fn a_character_is_the_one_the_layout_types_shift_or_not() {
        let chord = Chord::parse("Cmd+}").unwrap();
        let pressed = |key: &str, mods| Chord::pressed(&typed(key), mods).unwrap();
        assert!(chord.matches(&pressed("}", CMD | SHIFT)));
        assert!(chord.matches(&pressed("}", CMD)));
        assert!(!chord.matches(&pressed("]", CMD | SHIFT)));
    }

    #[test]
    fn a_character_written_with_shift_asks_for_shift() {
        let chord = Chord::parse("Ctrl+Shift+=").unwrap();
        let pressed = |mods| Chord::pressed(&typed("="), mods).unwrap();
        assert!(chord.matches(&pressed(CTRL | SHIFT)));
        assert!(!chord.matches(&pressed(CTRL)));
    }

    #[test]
    fn a_named_key_matches_with_exactly_the_modifiers_written() {
        let chord = Chord::parse("Ctrl+Shift+PageUp").unwrap();
        let page_up = Key::Named(NamedKey::PageUp);
        assert!(chord.matches(&Chord::pressed(&page_up, CTRL | SHIFT).unwrap()));
        assert!(!chord.matches(&Chord::pressed(&page_up, SHIFT).unwrap()));
    }

    #[test]
    fn a_user_s_key_takes_the_place_of_the_default_for_it() {
        let (keymap, problems) = Keymap::with_user("\"Ctrl+Alt+p\" = \"palette.open\"\n");
        assert_eq!(problems, []);
        assert_eq!(
            keymap.command(&typed("p"), CTRL | ALT),
            Some(Command::Palette)
        );
        // Other keys keep their defaults.
        assert!(keymap.chord(Command::Find).is_some());
    }

    #[test]
    fn a_user_s_key_rebinds_a_default_chord() {
        let default = Keymap::default().chord(Command::Find).unwrap().to_string();
        let raw = if cfg!(target_os = "macos") {
            "Cmd+f"
        } else {
            "Ctrl+Shift+f"
        };
        let (keymap, _) = Keymap::with_user(&format!("{raw:?} = \"palette.open\"\n"));
        let mods = Chord::parse(raw).unwrap().mods;
        assert_eq!(
            keymap.command(&typed("f"), mods),
            Some(Command::Palette),
            "{default}"
        );
    }

    #[test]
    fn none_takes_a_default_away() {
        let raw = if cfg!(target_os = "macos") {
            "Cmd+p"
        } else {
            "Ctrl+Shift+p"
        };
        let (keymap, problems) = Keymap::with_user(&format!("{raw:?} = \"none\"\n"));
        assert_eq!(problems, []);
        assert_eq!(keymap.chord(Command::Palette), None);
    }

    #[test]
    fn an_entry_that_does_not_read_is_told_and_the_rest_are_taken() {
        let (keymap, problems) = Keymap::with_user(
            "\"Hyper+a\" = \"palette.open\"\n\"Ctrl+Alt+a\" = \"nothing.such\"\n\"Ctrl+Alt+b\" = 3\n\"Ctrl+Alt+p\" = \"palette.open\"\n",
        );
        let keys: Vec<&str> = problems
            .iter()
            .map(|problem| problem.key.as_str())
            .collect();
        assert_eq!(keys, ["Hyper+a", "Ctrl+Alt+a", "Ctrl+Alt+b"]);
        assert_eq!(
            keymap.command(&typed("p"), CTRL | ALT),
            Some(Command::Palette)
        );
    }

    #[test]
    fn a_file_that_does_not_read_leaves_the_defaults() {
        let (keymap, problems) = Keymap::with_user("this is = = not toml");
        assert_eq!(problems.len(), 1);
        assert_eq!(keymap, Keymap::default());
    }

    #[test]
    fn a_chord_is_shown_as_the_platform_writes_chords() {
        let chord = Chord::parse("Ctrl+Shift+n").unwrap();
        let shown = if cfg!(target_os = "macos") {
            "⌃⇧N"
        } else {
            "Ctrl+Shift+N"
        };
        assert_eq!(chord.to_string(), shown);
    }

    #[test]
    fn the_keys_file_sits_beside_the_settings() {
        assert_eq!(
            path(Path::new("/home/me/.config/cattery/config.toml")),
            Path::new("/home/me/.config/cattery/keys.toml")
        );
    }
}
