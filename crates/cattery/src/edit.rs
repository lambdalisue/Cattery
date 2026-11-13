//! Changes the Preferences window makes to the settings file, written into
//! the file as it is so that everything else a user wrote there stays.

use felis_client_core::config::CursorBlinkMode;
use felis_render_core::palette::PALETTE_NAMES;
use toml_edit::{DocumentMut, Item, Table, TableLike, Value};

/// A colour as the settings file spells it: `#rrggbb`.
pub type Rgb = [u8; 3];

/// A colour the settings name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Foreground,
    Background,
    Cursor,
    /// One of the sixteen palette colours, by its index in felis's
    /// [`PALETTE_NAMES`].
    Palette(usize),
}

/// One change to the settings. `None` takes the setting out of the file, so
/// felis's default applies again.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    FontFamily(Option<String>),
    FontSize(Option<f32>),
    Color(Color, Option<Rgb>),
    Opacity(Option<f32>),
    Blink(CursorBlinkMode),
    /// The command a new session runs; empty runs the login shell.
    Shell(Vec<String>),
}

/// Make `edit` in `doc`.
pub fn apply(doc: &mut DocumentMut, edit: &Edit) {
    match edit {
        Edit::FontFamily(family) => {
            set(doc, &["font", "family"], family.as_deref().map(Value::from))
        }
        Edit::FontSize(size) => set(doc, &["font", "size_px"], size.map(float)),
        Edit::Color(color, rgb) => {
            let place: &[&str] = match color {
                Color::Foreground => &["theme", "foreground"],
                Color::Background => &["theme", "background"],
                Color::Cursor => &["cursor", "color"],
                Color::Palette(index) => &["theme", "palette", PALETTE_NAMES[*index]],
            };
            set(doc, place, rgb.map(|rgb| Value::from(hex(rgb))));
        }
        Edit::Opacity(opacity) => set(doc, &["window", "opacity"], opacity.map(float)),
        Edit::Blink(mode) => {
            let mode = match mode {
                CursorBlinkMode::Program => "program",
                CursorBlinkMode::Never => "never",
                CursorBlinkMode::Always => "always",
            };
            set(doc, &["cursor", "blink"], Some(Value::from(mode)));
        }
        Edit::Shell(command) => set(
            doc,
            &["shell", "command"],
            (!command.is_empty()).then(|| Value::Array(command.iter().collect())),
        ),
    }
}

/// `value` as the shortest decimal that reads back as the same `f32`, so
/// the file says `0.8` and not the `f64` nearest to that `f32`.
fn float(value: f32) -> Value {
    Value::from(value.to_string().parse::<f64>().unwrap_or(f64::from(value)))
}

/// Set the key at the end of `place`, in the tables the rest of `place`
/// names, or take it out when `value` is `None`.
fn set(doc: &mut DocumentMut, place: &[&str], value: Option<Value>) {
    let Some((key, tables)) = place.split_last() else {
        return;
    };
    match value {
        Some(value) => {
            let mut table: &mut dyn TableLike = doc.as_table_mut();
            for name in tables {
                table = table
                    .entry(name)
                    .or_insert_with(|| {
                        // An implicit table gets no header of its own while
                        // it holds only tables, as `theme` above
                        // `[theme.palette]`.
                        let mut new = Table::new();
                        new.set_implicit(true);
                        Item::Table(new)
                    })
                    .as_table_like_mut()
                    .expect("felis's settings sections are tables");
            }
            match table.get_mut(key).and_then(Item::as_value_mut) {
                // The comment after a value stays with the value.
                Some(old) => {
                    let decor = old.decor().clone();
                    *old = value;
                    *old.decor_mut() = decor;
                }
                None => {
                    table.insert(key, Item::Value(value));
                }
            }
        }
        None => remove(doc.as_table_mut(), tables, key),
    }
}

/// Take `key` out of the tables `tables` names, and each table it leaves
/// empty.
fn remove(table: &mut dyn TableLike, tables: &[&str], key: &str) {
    match tables.split_first() {
        None => {
            table.remove(key);
        }
        Some((name, rest)) => {
            let Some(inner) = table.get_mut(name).and_then(Item::as_table_like_mut) else {
                return;
            };
            remove(inner, rest, key);
            if inner.is_empty() {
                table.remove(name);
            }
        }
    }
}

/// The edit a command typed as a shell line makes; an empty line asks for
/// the login shell.
pub fn shell(line: &str) -> Result<Edit, shell_words::ParseError> {
    Ok(Edit::Shell(shell_words::split(line)?))
}

/// `command` as a shell line [`shell`] reads back.
pub fn shell_line(command: &[String]) -> String {
    shell_words::join(command)
}

/// `rgb` as the settings file spells it.
pub fn hex(rgb: Rgb) -> String {
    let [r, g, b] = rgb;
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// The colour `text` spells as `#rrggbb`, if it does.
pub fn rgb(text: &str) -> Option<Rgb> {
    let digits = text.strip_prefix('#').filter(|d| d.len() == 6)?;
    let channel = |at: usize| {
        let pair = digits.get(at..at + 2)?;
        pair.chars()
            .all(|c| c.is_ascii_hexdigit())
            .then(|| u8::from_str_radix(pair, 16).ok())?
    };
    Some([channel(0)?, channel(2)?, channel(4)?])
}

#[cfg(test)]
mod tests {
    use felis_client_core::config::{ConfigDocument, EffectiveConfig};

    use super::*;
    use crate::settings::CLIENT_ID;

    fn edited(text: &str, edits: &[Edit]) -> String {
        let mut doc: DocumentMut = text.parse().unwrap();
        for edit in edits {
            apply(&mut doc, edit);
        }
        doc.to_string()
    }

    fn read(text: &str) -> EffectiveConfig {
        let (settings, diagnostics) = ConfigDocument::parse(text, None)
            .unwrap()
            .resolve(CLIENT_ID);
        assert!(!diagnostics.has_errors(), "{text}");
        settings
    }

    #[test]
    fn a_setting_is_written_where_felis_reads_it() {
        let text = edited("", &[Edit::FontSize(Some(16.0))]);
        assert_eq!(read(&text).font.size_px, Some(16.0));
    }

    #[test]
    fn what_else_the_file_says_stays_as_written() {
        let before = "# my settings\n[font]\n# the face I like\nfamily = \"Iosevka\"\n\n[mouse]\nhide_while_typing = true\n";
        let after = edited(before, &[Edit::FontSize(Some(16.0))]);
        assert!(
            after.starts_with("# my settings\n[font]\n# the face I like\nfamily = \"Iosevka\"\n")
        );
        assert!(after.contains("[mouse]\nhide_while_typing = true\n"));
    }

    #[test]
    fn a_setting_is_changed_in_place() {
        let after = edited(
            "[font]\nsize_px = 12.0 # small\n",
            &[Edit::FontSize(Some(16.0))],
        );
        assert_eq!(after, "[font]\nsize_px = 16.0 # small\n");
    }

    #[test]
    fn a_cleared_setting_leaves_the_file() {
        let after = edited(
            "[font]\nfamily = \"Iosevka\"\nsize_px = 12.0\n",
            &[Edit::FontSize(None)],
        );
        assert_eq!(after, "[font]\nfamily = \"Iosevka\"\n");
    }

    #[test]
    fn a_section_left_empty_leaves_the_file() {
        let after = edited("[window]\nopacity = 0.8\n", &[Edit::Opacity(None)]);
        assert_eq!(after.trim(), "");
    }

    #[test]
    fn clearing_what_is_not_there_changes_nothing() {
        let before = "[font]\nfamily = \"Iosevka\"\n";
        assert_eq!(
            edited(before, &[Edit::Color(Color::Foreground, None)]),
            before
        );
    }

    #[test]
    fn each_setting_reaches_its_place_in_felis_s_settings() {
        let text = edited(
            "",
            &[
                Edit::FontFamily(Some("Iosevka".to_owned())),
                Edit::FontSize(Some(15.0)),
                Edit::Color(Color::Foreground, Some([0xc0, 0xc0, 0xc0])),
                Edit::Color(Color::Background, Some([0x10, 0x10, 0x10])),
                Edit::Color(Color::Cursor, Some([0, 0xff, 0])),
                Edit::Color(Color::Palette(9), Some([0xff, 0, 0])),
                Edit::Opacity(Some(0.75)),
                Edit::Blink(CursorBlinkMode::Never),
                Edit::Shell(vec!["/bin/zsh".to_owned(), "-l".to_owned()]),
            ],
        );
        let settings = read(&text);
        assert_eq!(settings.font.family.as_deref(), Some("Iosevka"));
        assert_eq!(settings.font.size_px, Some(15.0));
        assert_eq!(settings.theme.foreground.as_deref(), Some("#c0c0c0"));
        assert_eq!(settings.theme.background.as_deref(), Some("#101010"));
        assert_eq!(settings.cursor.color.as_deref(), Some("#00ff00"));
        assert_eq!(
            settings.theme.palette.bright_red.as_deref(),
            Some("#ff0000")
        );
        assert_eq!(settings.window.opacity, 0.75);
        assert_eq!(settings.cursor.blink, CursorBlinkMode::Never);
        assert_eq!(settings.shell.command, ["/bin/zsh", "-l"]);
    }

    #[test]
    fn the_login_shell_is_no_command_at_all() {
        let after = edited(
            "[shell]\ncommand = [\"/bin/zsh\"]\n",
            &[Edit::Shell(Vec::new())],
        );
        assert_eq!(after.trim(), "");
    }

    #[test]
    fn a_shell_line_is_split_as_a_shell_splits_it() {
        assert_eq!(
            shell("/bin/zsh -l -c 'echo hi'").unwrap(),
            Edit::Shell(vec![
                "/bin/zsh".into(),
                "-l".into(),
                "-c".into(),
                "echo hi".into()
            ]),
        );
    }

    #[test]
    fn an_empty_shell_line_asks_for_the_login_shell() {
        assert_eq!(shell("  ").unwrap(), Edit::Shell(Vec::new()));
    }

    #[test]
    fn an_unfinished_quote_is_no_command() {
        assert!(shell("zsh -c 'echo").is_err());
    }

    #[test]
    fn a_command_reads_back_from_its_shell_line() {
        let command = vec!["/opt/my shell/bin/fish".to_owned(), "--login".to_owned()];
        assert_eq!(shell(&shell_line(&command)).unwrap(), Edit::Shell(command));
    }

    #[test]
    fn a_colour_is_spelled_as_six_hex_digits() {
        assert_eq!(hex([0x0a, 0xbc, 0xff]), "#0abcff");
        assert_eq!(rgb("#0abcff"), Some([0x0a, 0xbc, 0xff]));
        assert_eq!(rgb("#0ABCFF"), Some([0x0a, 0xbc, 0xff]));
    }

    #[test]
    fn other_spellings_are_not_read_as_colours() {
        for text in ["red", "0abcff", "#abc", "#0abcfff", "#0abcfg", ""] {
            assert_eq!(rgb(text), None, "{text}");
        }
    }
}
