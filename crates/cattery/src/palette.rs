//! The command palette: the sessions Cattery's daemon keeps, to open in a
//! window again or to end, and Cattery's commands, narrowed by what is
//! typed, with what is known of the session chosen beside them.

use std::time::SystemTime;

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align, CentralPanel, Color32, CornerRadius, FontId, Frame, Key, Layout, Margin, RichText,
    ScrollArea, Sense, Stroke, TextEdit, Ui, pos2, vec2,
};
use tokio::runtime::Runtime;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use crate::commands;
use crate::fuzzy::{self, Query};
use crate::keymap::Keymap;
use crate::keys::Command;
use crate::look::Tokens;
use crate::roster::{self, Outcome, Row, State};
use crate::surface::Surface;

/// What a window event asks of the app.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Nothing,
    /// Run the command, as the window in front would.
    Run(Command),
    /// Show the session in a window.
    Open(u128),
    /// End the session and its program.
    End(u128),
    Closed,
}

/// The palette's size, in logical pixels.
const SIZE: LogicalSize<f64> = LogicalSize::new(760.0, 440.0);

/// The window a palette opens over: where it is, how big, and at what scale.
pub type Over = (PhysicalPosition<i32>, PhysicalSize<u32>, f64);

/// What an entry stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Session(u128),
    Command(Command),
}

/// One entry of the list.
#[derive(Debug)]
struct Entry<'a> {
    choice: Choice,
    title: String,
    /// The line under the title.
    detail: Vec<String>,
    /// What is shown at the right: a command's chord.
    hint: Option<String>,
    /// The characters of the title the query matched.
    marks: Vec<usize>,
    row: Option<&'a Row>,
}

pub struct Palette {
    surface: Surface,
    rows: Vec<Row>,
    query: String,
    /// The entry chosen, counted down the sessions and then the commands.
    chosen: usize,
    /// Whether the palette has been in front.
    focused: bool,
}

impl Palette {
    /// Open the palette over the window `over`, where there is one.
    pub fn open(
        event_loop: &ActiveEventLoop,
        runtime: &Runtime,
        over: Option<Over>,
    ) -> anyhow::Result<Self> {
        let mut attributes = Window::default_attributes()
            .with_title("Cattery")
            .with_inner_size(SIZE)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop);
        if let Some(over) = over {
            attributes = attributes.with_position(placed(over));
        }
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS as _;
            // A title bar left in place but hidden keeps macOS's rounded
            // corners and shadow.
            attributes = attributes
                .with_titlebar_transparent(true)
                .with_title_hidden(true)
                .with_titlebar_buttons_hidden(true)
                .with_fullsize_content_view(true);
        }
        #[cfg(not(target_os = "macos"))]
        {
            attributes = attributes.with_decorations(false);
        }
        let surface = Surface::open(event_loop, runtime, attributes)?;
        Ok(Self {
            surface,
            rows: Vec::new(),
            query: String::new(),
            chosen: 0,
            focused: false,
        })
    }

    pub fn id(&self) -> WindowId {
        self.surface.id()
    }

    pub fn focus(&self) {
        self.surface.focus();
    }

    /// List `rows` from now on; the first sessions listed start on
    /// `previous`.
    pub fn show(&mut self, rows: Vec<Row>, previous: Option<u128>) {
        if rows != self.rows {
            // The same entry stays chosen as the list changes under it.
            let now = SystemTime::now();
            let was = if self.rows.is_empty() {
                previous.map(Choice::Session)
            } else {
                let (sessions, commands) = entries(&self.rows, &self.query, now);
                at(&sessions, &commands, self.chosen).map(|entry| entry.choice)
            };
            self.rows = rows;
            if let Some(position) = position(&self.rows, &self.query, was, now) {
                self.chosen = position;
            }
            self.surface.redraw();
        }
    }

    pub fn window_event(&mut self, event: &WindowEvent, keymap: &Keymap) -> Asked {
        match event {
            WindowEvent::CloseRequested => return Asked::Closed,
            WindowEvent::Focused(true) => self.focused = true,
            // A palette is gone once the user turns elsewhere, which is not
            // yet so before it was ever in front.
            WindowEvent::Focused(false) if self.focused => return Asked::Closed,
            _ => {}
        }
        if !self.surface.take(event) {
            return Asked::Nothing;
        }
        let (rows, query, chosen) = (&self.rows, &mut self.query, &mut self.chosen);
        let mut asked = Asked::Nothing;
        self.surface
            .frame(|ui| asked = palette(ui, rows, query, chosen, keymap));
        asked
    }
}

/// Where the palette goes over the window `over`: centred across it, a
/// fifth of the way down.
fn placed((position, size, scale): Over) -> PhysicalPosition<i32> {
    let width = SIZE.width * scale;
    let offset = |value: f64| -> i32 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a window is a few thousand pixels"
        )]
        let offset = value.round() as i32;
        offset
    };
    PhysicalPosition::new(
        position.x + offset((f64::from(size.width) - width) / 2.0),
        position.y + offset(f64::from(size.height) / 5.0),
    )
}

/// The entries matching `query`: the sessions, then the commands, each
/// best first, or as listed while nothing is typed.
fn entries<'a>(rows: &'a [Row], query: &str, now: SystemTime) -> (Vec<Entry<'a>>, Vec<Entry<'a>>) {
    let mut matching = Query::new(query);
    let mut sessions: Vec<(u32, Entry<'a>)> = rows
        .iter()
        .filter_map(|row| {
            let titled = matching.find(&row.title);
            // A session is known by its name first; where it is and what
            // runs in it match too, at less weight.
            let score = [
                titled.as_ref().map(|found| found.score.saturating_mul(2)),
                matching.find(&row.place).map(|found| found.score),
                row.process
                    .as_deref()
                    .and_then(|process| matching.find(process))
                    .map(|found| found.score),
            ]
            .into_iter()
            .flatten()
            .max()?;
            Some((
                score,
                Entry {
                    choice: Choice::Session(row.id),
                    title: row.title.clone(),
                    detail: summary(row, now),
                    hint: Some(used(row.state, now)),
                    marks: titled.map(|found| found.marks).unwrap_or_default(),
                    row: Some(row),
                },
            ))
        })
        .collect();
    if !matching.is_empty() {
        sessions.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    }
    let commands = fuzzy::ranked(
        &mut matching,
        commands::LISTED
            .iter()
            .map(|command| commands::label(*command)),
    )
    .into_iter()
    .map(|(at, found)| {
        let command = commands::LISTED[at];
        Entry {
            choice: Choice::Command(command),
            title: commands::label(command).to_owned(),
            detail: Vec::new(),
            hint: None,
            marks: found.marks,
            row: None,
        }
    })
    .collect();
    (
        sessions.into_iter().map(|(_, entry)| entry).collect(),
        commands,
    )
}

/// The entry `chosen` counts to, down the sessions and then the commands.
fn at<'e, 'a>(
    sessions: &'e [Entry<'a>],
    commands: &'e [Entry<'a>],
    chosen: usize,
) -> Option<&'e Entry<'a>> {
    sessions.iter().chain(commands).nth(chosen)
}

/// Where `choice` stands among the entries matching `query`.
fn position(rows: &[Row], query: &str, choice: Option<Choice>, now: SystemTime) -> Option<usize> {
    let choice = choice?;
    let (sessions, commands) = entries(rows, query, now);
    sessions
        .iter()
        .chain(&commands)
        .position(|entry| entry.choice == choice)
}

/// The lines under a session's title: where it is and what runs in it;
/// then how its last command went, when it was created, and its size.
fn summary(row: &Row, now: SystemTime) -> Vec<String> {
    let joined = |parts: Vec<Option<String>>| -> Option<String> {
        let parts: Vec<String> = parts.into_iter().flatten().collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    };
    [
        joined(vec![
            Some(row.place.clone()).filter(|place| !place.is_empty()),
            row.process.clone(),
        ]),
        joined(vec![
            briefly(row),
            row.created
                .map(|created| format!("Created {}", roster::ago(created, now).to_lowercase())),
            Some(format!("{}×{}", row.size.0, row.size.1)),
        ]),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// A session's last command in a few characters: a mark for how it went,
/// the line, and the code it failed with.
fn briefly(row: &Row) -> Option<String> {
    let running = row.command.as_ref().map(|ran| ran.outcome) == Some(Outcome::Running);
    let failed = match (row.command.as_ref().map(|ran| ran.outcome), row.exit) {
        (Some(Outcome::Running), _) => false,
        (_, Some(code)) => code != 0,
        (Some(outcome), None) => outcome == Outcome::Failed,
        (None, None) => return None,
    };
    let mark = if running {
        "▶"
    } else if failed {
        "✗"
    } else {
        "✓"
    };
    let code = row.exit.filter(|code| failed && *code != 0);
    Some(match (&row.command, code) {
        (Some(ran), Some(code)) => format!("{mark} {} ({code})", ran.line),
        (Some(ran), None) => format!("{mark} {}", ran.line),
        (None, Some(code)) => format!("{mark} exit {code}"),
        (None, None) => mark.to_owned(),
    })
}

/// When a session in `state` was last used.
fn used(state: State, now: SystemTime) -> String {
    match state {
        State::Shown(1) => "Open in a window".to_owned(),
        State::Shown(windows) => format!("Open in {windows} windows"),
        State::Background(idle) => {
            format!("Used {}", roster::ago(now - idle, now).to_lowercase())
        }
        State::Ended => "Ended".to_owned(),
    }
}

/// What is known of the session `row`, as labelled lines.
fn details(row: &Row, now: SystemTime) -> Vec<(&'static str, String)> {
    let unknown = || "Unknown".to_owned();
    vec![
        (
            "Directory",
            Some(row.place.clone())
                .filter(|place| !place.is_empty())
                .unwrap_or_else(unknown),
        ),
        ("Started in", row.started_in.clone().unwrap_or_else(unknown)),
        ("Running", row.process.clone().unwrap_or_else(unknown)),
        (
            "Created",
            row.created
                .map_or_else(unknown, |created| roster::ago(created, now)),
        ),
        ("Last used", used(row.state, now)),
        ("Last command", last_command(row)),
        ("Size", format!("{} × {}", row.size.0, row.size.1)),
    ]
}

/// The last command a session's shell reported, and how it went.
fn last_command(row: &Row) -> String {
    let outcome = match (row.command.as_ref().map(|ran| ran.outcome), row.exit) {
        (Some(Outcome::Running), _) => "running".to_owned(),
        (_, Some(0)) | (Some(Outcome::Succeeded), None) => "succeeded".to_owned(),
        (_, Some(code)) => format!("failed ({code})"),
        (Some(Outcome::Failed), None) => "failed".to_owned(),
        (None, None) => return "Not reported".to_owned(),
    };
    match &row.command {
        Some(ran) => format!("{} — {outcome}", ran.line),
        None => capitalized(&outcome),
    }
}

fn capitalized(text: &str) -> String {
    let mut letters = text.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

/// How far a key moves the choice: down, up, or not at all.
fn step(input: &mut egui::InputState) -> Option<isize> {
    let none = egui::Modifiers::NONE;
    let ctrl = egui::Modifiers::CTRL;
    let down = [
        (none, Key::ArrowDown),
        (none, Key::Tab),
        (ctrl, Key::N),
        (ctrl, Key::J),
    ];
    let up = [
        (none, Key::ArrowUp),
        (egui::Modifiers::SHIFT, Key::Tab),
        (ctrl, Key::P),
        (ctrl, Key::K),
    ];
    // Every key is taken, so none of them reaches the query field.
    let down = down.iter().fold(false, |any, (mods, key)| {
        input.consume_key(*mods, *key) || any
    });
    let up = up.iter().fold(false, |any, (mods, key)| {
        input.consume_key(*mods, *key) || any
    });
    match (down, up) {
        (true, false) => Some(1),
        (false, true) => Some(-1),
        _ => None,
    }
}

/// `chosen` moved by `by` among `count` entries, wrapping round.
fn moved(chosen: usize, by: isize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let count = isize::try_from(count).unwrap_or(isize::MAX);
    let chosen = isize::try_from(chosen).unwrap_or(0);
    usize::try_from((chosen + by).rem_euclid(count)).unwrap_or(0)
}

fn palette(
    ui: &mut Ui,
    rows: &[Row],
    query: &mut String,
    chosen: &mut usize,
    keymap: &Keymap,
) -> Asked {
    let tokens = Tokens::of(ui.visuals().dark_mode);
    let now = SystemTime::now();
    let (sessions, mut commands) = entries(rows, query, now);
    for entry in &mut commands {
        if let Choice::Command(command) = entry.choice {
            entry.hint = keymap.chord(command).map(|chord| chord.to_string());
        }
    }
    let count = sessions.len() + commands.len();
    *chosen = (*chosen).min(count.saturating_sub(1));
    let end_chord = if cfg!(target_os = "macos") {
        egui::Modifiers::COMMAND
    } else {
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT
    };
    let mut asked = Asked::Nothing;
    ui.input_mut(|input| {
        if let Some(by) = step(input) {
            *chosen = moved(*chosen, by, count);
        }
        if input.consume_key(egui::Modifiers::NONE, Key::Escape)
            || input.consume_key(egui::Modifiers::CTRL, Key::C)
        {
            asked = Asked::Closed;
        }
        if input.consume_key(end_chord, Key::Backspace)
            && let Some(Choice::Session(id)) =
                at(&sessions, &commands, *chosen).map(|entry| entry.choice)
        {
            asked = Asked::End(id);
        }
        if input.consume_key(egui::Modifiers::NONE, Key::Enter)
            && let Some(entry) = at(&sessions, &commands, *chosen)
        {
            asked = chosen_asks(entry.choice);
        }
    });
    CentralPanel::default()
        .frame(
            Frame::new()
                .fill(tokens.group)
                .inner_margin(Margin::symmetric(12, 12)),
        )
        .show(ui, |ui| {
            // The macOS title bar is hidden over the top of the palette.
            if cfg!(target_os = "macos") {
                ui.add_space(14.0);
            }
            let typed = ui.add(
                TextEdit::singleline(query)
                    .hint_text("Search sessions and commands")
                    .font(FontId::proportional(17.0))
                    .frame(Frame::NONE)
                    .desired_width(f32::INFINITY),
            );
            typed.request_focus();
            if typed.changed() {
                *chosen = 0;
            }
            ui.add_space(8.0);
            rule(ui, tokens);
            ui.add_space(6.0);
            let footer = 22.0;
            let height = ui.available_height() - footer;
            let width = ui.available_width();
            ui.allocate_ui_with_layout(
                vec2(width, height),
                Layout::left_to_right(Align::Min),
                |ui| {
                    let list = width * 0.58;
                    ui.allocate_ui_with_layout(
                        vec2(list, height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ScrollArea::vertical()
                                .id_salt("entries")
                                .max_height(height)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut index = 0;
                                    for (heading, group) in
                                        [("Sessions", &sessions), ("Commands", &commands)]
                                    {
                                        if group.is_empty() {
                                            continue;
                                        }
                                        ui.label(
                                            RichText::new(heading).size(11.0).color(tokens.muted),
                                        );
                                        for entry in group.iter() {
                                            let response =
                                                item(ui, tokens, *chosen == index, entry);
                                            if response.hovered() {
                                                *chosen = index;
                                            }
                                            if response.clicked() {
                                                asked = chosen_asks(entry.choice);
                                            }
                                            index += 1;
                                        }
                                        ui.add_space(6.0);
                                    }
                                    if count == 0 {
                                        ui.label(
                                            RichText::new("Nothing matches").color(tokens.muted),
                                        );
                                    }
                                });
                        },
                    );
                    ui.add_space(12.0);
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), height),
                        Layout::top_down(Align::Min),
                        |ui| side(ui, tokens, at(&sessions, &commands, *chosen), now),
                    );
                },
            );
            ui.add_space(4.0);
            let (open, end) = if cfg!(target_os = "macos") {
                ("Return", "⌘⌫")
            } else {
                ("Enter", "Ctrl+Shift+Backspace")
            };
            ui.label(
                RichText::new(format!(
                    "Ctrl+N/P Move   {open} Choose   {end} End session   Esc Close"
                ))
                .small()
                .color(tokens.muted),
            );
        });
    asked
}

/// What choosing `choice` asks for.
fn chosen_asks(choice: Choice) -> Asked {
    match choice {
        Choice::Session(id) => Asked::Open(id),
        Choice::Command(command) => Asked::Run(command),
    }
}

fn rule(ui: &mut Ui, tokens: Tokens) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, tokens.separator),
    );
}

/// The pane beside the list: what is known of the session chosen, or what
/// the command chosen is.
fn side(ui: &mut Ui, tokens: Tokens, entry: Option<&Entry<'_>>, now: SystemTime) {
    let Some(entry) = entry else {
        return;
    };
    ui.label(
        RichText::new(&entry.title)
            .size(14.0)
            .color(tokens.text)
            .strong(),
    );
    ui.add_space(8.0);
    if let Some(row) = entry.row {
        for (label, value) in details(row, now) {
            ui.label(RichText::new(label).size(11.0).color(tokens.muted));
            ui.label(RichText::new(value).size(12.0).color(tokens.text));
            ui.add_space(4.0);
        }
    } else if let Some(chord) = entry.hint.as_deref() {
        ui.label(RichText::new("Shortcut").size(11.0).color(tokens.muted));
        ui.label(RichText::new(chord).size(12.0).color(tokens.text));
    }
}

/// One entry: its title, matched letters marked, the lines under it, and
/// its hint at the right of the title; drawn as chosen when it is.
fn item(ui: &mut Ui, tokens: Tokens, chosen: bool, entry: &Entry<'_>) -> egui::Response {
    #[expect(clippy::cast_precision_loss, reason = "an entry has a few lines")]
    let tall = 16.0f32.mul_add(entry.detail.len() as f32, 26.0);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), tall), Sense::click());
    let (text, muted, marked) = if chosen {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), tokens.accent);
        (
            tokens.on_accent,
            tokens.on_accent.gamma_multiply(0.8),
            tokens.on_accent,
        )
    } else {
        (tokens.text, tokens.muted, tokens.accent)
    };
    let left = rect.left() + 10.0;
    let hint = entry.hint.as_deref().map(|hint| {
        ui.painter()
            .layout_no_wrap(hint.to_owned(), FontId::proportional(12.0), muted)
    });
    // The text stops short of the hint, with room between them.
    let width = rect.width() - 20.0 - hint.as_ref().map_or(0.0, |hint| hint.size().x + 12.0);
    let title = ui.painter().layout_job(one_line(
        marked_title(&entry.title, &entry.marks, text, marked),
        width,
    ));
    let top = if entry.detail.is_empty() {
        rect.center().y
    } else {
        rect.top() + 13.0
    };
    ui.painter()
        .galley(pos2(left, top - title.size().y / 2.0), title, text);
    let mut middle = rect.top() + 29.0;
    for line in &entry.detail {
        let detail = ui.painter().layout_job(one_line(
            LayoutJob::simple_singleline(line.clone(), FontId::proportional(11.0), muted),
            rect.width() - 20.0,
        ));
        ui.painter()
            .galley(pos2(left, middle - detail.size().y / 2.0), detail, muted);
        middle += 16.0;
    }
    if let Some(hint) = hint {
        let at = pos2(
            rect.right() - 10.0 - hint.size().x,
            top - hint.size().y / 2.0,
        );
        ui.painter().galley(at, hint, muted);
    }
    if chosen {
        ui.scroll_to_rect(rect, None);
    }
    response
}

/// `job` on one line no wider than `width`, what does not fit left out
/// behind an ellipsis.
pub fn one_line(mut job: LayoutJob, width: f32) -> LayoutJob {
    job.wrap = TextWrapping {
        max_width: width.max(0.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    job
}

/// `title` with the characters at `marks` drawn in `marked`, bold.
fn marked_title(title: &str, marks: &[usize], text: Color32, marked: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let plain = TextFormat::simple(FontId::proportional(13.0), text);
    let strong = TextFormat::simple(crate::look::bold(13.0), marked);
    for (at, letter) in title.chars().enumerate() {
        let format = if marks.binary_search(&at).is_ok() {
            strong.clone()
        } else {
            plain.clone()
        };
        job.append(letter.encode_utf8(&mut [0; 4]), 0.0, format);
    }
    job
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::settings::Zoom;

    #[test]
    fn an_entry_s_text_keeps_to_one_line_of_its_row_behind_an_ellipsis() {
        let wrap = one_line(LayoutJob::default(), 120.0).wrap;
        assert_eq!((wrap.max_width, wrap.max_rows), (120.0, 1));
        assert!(wrap.break_anywhere);
        assert_eq!(wrap.overflow_character, Some('…'));
    }

    #[test]
    fn a_row_too_narrow_for_any_text_takes_none() {
        assert_eq!(one_line(LayoutJob::default(), -5.0).wrap.max_width, 0.0);
    }

    fn row(id: u128, title: &str) -> Row {
        Row {
            id,
            title: title.to_owned(),
            place: "~/src".to_owned(),
            state: State::Background(Duration::from_secs(120)),
            process: Some("zsh".to_owned()),
            started_in: Some("~".to_owned()),
            created: None,
            exit: None,
            command: None,
            size: (80, 24),
        }
    }

    fn choices(entries: &[Entry<'_>]) -> Vec<Choice> {
        entries.iter().map(|entry| entry.choice).collect()
    }

    #[test]
    fn nothing_typed_lists_every_session_and_then_every_command() {
        let rows = [row(1, "vim"), row(2, "cargo")];
        let (sessions, commands) = entries(&rows, "", SystemTime::now());
        assert_eq!(choices(&sessions), [Choice::Session(1), Choice::Session(2)]);
        assert_eq!(commands.len(), commands::LISTED.len());
        assert_eq!(commands[0].choice, Choice::Command(Command::NewWindow));
    }

    #[test]
    fn an_entry_is_found_where_the_list_holds_it() {
        let rows = [row(1, "vim"), row(2, "cargo")];
        let now = SystemTime::now();
        assert_eq!(position(&rows, "", Some(Choice::Session(2)), now), Some(1));
        assert_eq!(
            position(&rows, "", Some(Choice::Command(Command::NewWindow)), now),
            Some(2)
        );
        assert_eq!(position(&rows, "", Some(Choice::Session(9)), now), None);
        assert_eq!(position(&rows, "", None, now), None);
    }

    #[test]
    fn what_is_typed_narrows_sessions_and_commands_alike() {
        let rows = [row(1, "vim notes.md"), row(2, "cargo build")];
        let (sessions, commands) = entries(&rows, "bigger", SystemTime::now());
        assert!(sessions.is_empty());
        assert_eq!(
            choices(&commands),
            [Choice::Command(Command::Zoom(Zoom::In))]
        );
        let (sessions, _) = entries(&rows, "vim", SystemTime::now());
        assert_eq!(choices(&sessions), [Choice::Session(1)]);
    }

    #[test]
    fn a_session_is_found_by_where_it_is_and_what_runs_in_it() {
        let mut elsewhere = row(1, "Shell");
        elsewhere.place = "~/src/cattery".to_owned();
        elsewhere.process = Some("nvim".to_owned());
        let rows = [elsewhere, row(2, "Shell")];
        assert_eq!(
            choices(&entries(&rows, "cattery", SystemTime::now()).0),
            [Choice::Session(1)]
        );
        assert_eq!(
            choices(&entries(&rows, "nvim", SystemTime::now()).0),
            [Choice::Session(1)]
        );
    }

    #[test]
    fn a_session_s_name_counts_for_more_than_where_it_is() {
        let mut named = row(1, "cattery");
        named.place = "~".to_owned();
        let mut placed = row(2, "Shell");
        placed.place = "~/cattery".to_owned();
        let rows = [placed, named];
        assert_eq!(
            choices(&entries(&rows, "cattery", SystemTime::now()).0),
            [Choice::Session(1), Choice::Session(2)]
        );
    }

    #[test]
    fn a_session_says_where_it_is_and_what_runs_then_how_its_last_command_went() {
        let now = SystemTime::now();
        let mut busy = row(1, "vim");
        busy.created = Some(now - Duration::from_secs(3 * 3600));
        busy.command = Some(roster::Ran {
            line: "cargo test".to_owned(),
            outcome: Outcome::Succeeded,
        });
        assert_eq!(
            summary(&busy, now),
            ["~/src · zsh", "✓ cargo test · Created 3 h ago · 80×24"]
        );
    }

    #[test]
    fn a_session_little_is_known_of_still_tells_its_size() {
        let mut bare = row(1, "zsh");
        bare.place = String::new();
        bare.process = None;
        assert_eq!(summary(&bare, SystemTime::now()), ["80×24"]);
    }

    #[test]
    fn a_session_s_entry_says_at_its_right_when_it_was_used() {
        let rows = [row(1, "vim")];
        let (sessions, _) = entries(&rows, "", SystemTime::now());
        assert_eq!(sessions[0].hint.as_deref(), Some("Used 2 min ago"));
    }

    #[test]
    fn the_last_command_is_told_briefly_with_a_mark_for_how_it_went() {
        let mut shown = row(1, "zsh");
        assert_eq!(briefly(&shown), None);
        shown.exit = Some(2);
        assert_eq!(briefly(&shown).as_deref(), Some("✗ exit 2"));
        shown.command = Some(roster::Ran {
            line: "make".to_owned(),
            outcome: Outcome::Failed,
        });
        assert_eq!(briefly(&shown).as_deref(), Some("✗ make (2)"));
        shown.exit = None;
        shown.command = Some(roster::Ran {
            line: "cargo build".to_owned(),
            outcome: Outcome::Running,
        });
        assert_eq!(briefly(&shown).as_deref(), Some("▶ cargo build"));
        shown.exit = Some(0);
        shown.command = Some(roster::Ran {
            line: "ls".to_owned(),
            outcome: Outcome::Succeeded,
        });
        assert_eq!(briefly(&shown).as_deref(), Some("✓ ls"));
    }

    #[test]
    fn the_session_chosen_is_described_in_full() {
        let now = SystemTime::now();
        let mut described = row(1, "vim");
        described.created = Some(now - Duration::from_secs(3 * 3600));
        described.exit = Some(1);
        described.command = Some(roster::Ran {
            line: "cargo test".to_owned(),
            outcome: Outcome::Failed,
        });
        let details = details(&described, now);
        let value = |label: &str| {
            details
                .iter()
                .find(|(each, _)| *each == label)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(value("Directory"), Some("~/src"));
        assert_eq!(value("Started in"), Some("~"));
        assert_eq!(value("Running"), Some("zsh"));
        assert_eq!(value("Created"), Some("3 h ago"));
        assert_eq!(value("Last used"), Some("Used 2 min ago"));
        assert_eq!(value("Last command"), Some("cargo test — failed (1)"));
        assert_eq!(value("Size"), Some("80 × 24"));
    }

    #[test]
    fn the_last_command_says_how_it_went_as_far_as_is_known() {
        let mut shown = row(1, "zsh");
        assert_eq!(last_command(&shown), "Not reported");
        shown.exit = Some(0);
        assert_eq!(last_command(&shown), "Succeeded");
        shown.command = Some(roster::Ran {
            line: "cargo build".to_owned(),
            outcome: Outcome::Running,
        });
        assert_eq!(last_command(&shown), "cargo build — running");
    }

    #[test]
    fn choosing_a_session_opens_it_and_a_command_runs_it() {
        assert_eq!(chosen_asks(Choice::Session(7)), Asked::Open(7));
        assert_eq!(
            chosen_asks(Choice::Command(Command::Find)),
            Asked::Run(Command::Find)
        );
    }

    #[test]
    fn moving_past_either_end_wraps_round() {
        assert_eq!(moved(0, -1, 3), 2);
        assert_eq!(moved(2, 1, 3), 0);
        assert_eq!(moved(1, 1, 3), 2);
        assert_eq!(moved(0, 1, 0), 0);
    }

    #[test]
    fn the_palette_sits_centred_a_fifth_of_the_way_down_the_window() {
        let at = placed((
            PhysicalPosition::new(100, 50),
            PhysicalSize::new(1920, 1000),
            2.0,
        ));
        assert_eq!(at, PhysicalPosition::new(100 + 200, 50 + 200));
    }
}
