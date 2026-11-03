//! Drawing. A pure function of the `App`, so it is tested against a `TestBackend` buffer.

use crate::app::{App, Screen, Section};
use crate::flow::PromptKind;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;

fn red() -> Style {
    Style::new().fg(Color::Red)
}

fn section_name(s: Section) -> &'static str {
    match s {
        Section::Disk => "Disk",
        Section::Boot => "Boot",
        Section::System => "System",
        Section::User => "User",
        Section::Profile => "Profile",
        Section::Network => "Network",
        Section::Extras => "Extras",
    }
}

/// The lines of `all` that fit in `height`, `up` lines before the end.
fn tail(all: &[String], height: usize, up: usize) -> Vec<Line<'static>> {
    let end = all.len().saturating_sub(up.min(all.len()));
    let start = end.saturating_sub(height);
    all[start..end]
        .iter()
        .map(|l| Line::from(l.clone()))
        .collect()
}

pub fn draw(app: &App, f: &mut Frame) {
    let area = f.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        f.render_widget(Paragraph::new("terminal too small (need 80x24)"), area);
        return;
    }
    let [title, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .areas(area);

    let name = match app.screen {
        Screen::Menu => "main menu".to_owned(),
        Screen::Section(s) => section_name(s).to_lowercase(),
        Screen::Review => "review the plan".into(),
        Screen::Confirm => "confirm".into(),
        Screen::Run => "installing".into(),
        Screen::Done => "done".into(),
    };
    f.render_widget(Paragraph::new(format!("arch-to-go · {name}")), title);

    let lines = match app.screen {
        Screen::Menu => menu_lines(app),
        Screen::Section(s) => section_lines(app, s),
        Screen::Review => app
            .review
            .iter()
            .skip(app.review_scroll)
            .take(body.height as usize)
            .map(|l| Line::from(l.clone()))
            .collect(),
        Screen::Confirm => confirm_lines(app),
        Screen::Run => run_lines(app, body),
        Screen::Done => done_lines(app, body),
    };
    f.render_widget(Paragraph::new(lines), body);

    let hint = match app.screen {
        Screen::Menu => "↑/↓ move · Enter open · q quit",
        Screen::Section(_) => "↑/↓ move · Enter edit/toggle · ←/→ change · Space toggle · Esc back",
        Screen::Review => "↑/↓ PgUp/PgDn scroll · Esc back",
        Screen::Confirm => "type the device path · Enter confirm · Esc back",
        Screen::Run => "Ctrl-C abort · ↑/↓ scroll the log",
        Screen::Done => "Enter back to the menu · q quit",
    };
    let msg = app.message.clone().unwrap_or_default();
    f.render_widget(
        Paragraph::new(vec![Line::styled(msg, red()), Line::from(hint)]),
        footer,
    );
}

fn menu_lines(app: &App) -> Vec<Line<'static>> {
    let mut out: Vec<Line> = app
        .menu_items()
        .into_iter()
        .enumerate()
        .map(|(i, m)| {
            let cur = if i == app.menu_cursor() { '>' } else { ' ' };
            let mark = if m.invalid { '!' } else { ' ' };
            let line = format!("{cur} {mark} {:<10} {}", m.label, m.value);
            if m.invalid {
                Line::styled(line, red())
            } else {
                Line::from(line)
            }
        })
        .collect();
    if let Some((kind, text)) = app.prompt() {
        let what = match kind {
            PromptKind::Save => "Save config to",
            PromptKind::Load => "Load config from",
        };
        out.push(Line::from(""));
        out.push(Line::from(format!("{what}: {text}_")));
    }
    out
}

fn section_lines(app: &App, s: Section) -> Vec<Line<'static>> {
    let mut out = vec![Line::from(section_name(s)), Line::from("")];
    for (i, r) in app.rows(s).into_iter().enumerate() {
        let cur = if i == app.cursor() { '>' } else { ' ' };
        let end = if r.editing { "_" } else { "" };
        out.push(Line::from(format!(
            "{cur} {:<26} {}{end}",
            r.label, r.value
        )));
    }
    if s == Section::Disk && app.disks.is_empty() {
        out.push(Line::from(""));
        out.push(Line::styled("no USB disk found", red()));
    }
    out
}

fn confirm_lines(app: &App) -> Vec<Line<'static>> {
    let dev = app.device_label().unwrap_or_default();
    vec![
        Line::styled(format!("This will ERASE everything on {dev}."), red()),
        Line::from(""),
        Line::from("Type the exact device path to continue:"),
        Line::from(format!("{}_", app.confirm_text())),
    ]
}

fn run_lines(app: &App, body: Rect) -> Vec<Line<'static>> {
    let step = app.step.clone().unwrap_or_else(|| "starting".into());
    let mut out = vec![Line::from(format!("Step: {step}")), Line::from("")];
    out.extend(tail(
        &app.log,
        (body.height as usize).saturating_sub(2),
        app.log_scroll,
    ));
    out
}

fn done_lines(app: &App, body: Rect) -> Vec<Line<'static>> {
    let head = match app.exit_code {
        Some(0) => Line::styled(
            "Install finished successfully (exit 0)",
            Style::new().fg(Color::Green),
        ),
        Some(c) => Line::styled(format!("Install failed (exit {c})"), red()),
        None => Line::from("Install ended"),
    };
    let mut out = vec![head, Line::from("")];
    out.extend(tail(
        &app.log,
        (body.height as usize).saturating_sub(2),
        app.log_scroll,
    ));
    out
}
