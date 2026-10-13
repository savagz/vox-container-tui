use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::config::{ConfigField, ConfigForm};
use crate::i18n::{self, Key};
use crate::theme::Theme;

use super::widgets::panel;

const LABEL_WIDTH: usize = 24;

/// Overlay centrado sobre el tablero: intervalos de polling, timeout de
/// comandos e idioma de la interfaz — no hay credenciales que pedir.
pub fn render(frame: &mut Frame, area: Rect, form: &ConfigForm, theme: &Theme) {
    let lang = form.language;
    let modal = panel::centered(area, 62, 40);
    frame.render_widget(Clear, modal);

    let block = panel::block(theme, i18n::t(lang, Key::ConfigFormTitle), theme.accent_primary, true);
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(ConfigField::ALL.len() as u16),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(inner);

    render_fields(frame, rows[0], form, theme);
    render_hint(frame, rows[1], form, theme);
    render_error(frame, rows[2], form, theme);
    render_keys(frame, rows[3], theme, lang);
}

fn render_fields(frame: &mut Frame, area: Rect, form: &ConfigForm, theme: &Theme) {
    let lines: Vec<Line> = ConfigField::ALL
        .iter()
        .map(|field| field_line(*field, form, theme))
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn field_line<'a>(field: ConfigField, form: &ConfigForm, theme: &Theme) -> Line<'a> {
    let focused = form.field() == field;
    let marker = if focused { "▌" } else { " " };
    let label = format!("{:<width$}", field.label(form.language), width = LABEL_WIDTH);
    let value_style = if focused {
        theme.selected_row_style()
    } else {
        theme.value_style()
    };

    let mut spans = vec![
        Span::styled(marker.to_string(), theme.style_for_color(theme.accent_primary)),
        Span::styled(
            label,
            if focused { theme.style_for_bold() } else { theme.label_style() },
        ),
        Span::styled("│ ", theme.style_for_color(theme.border)),
        Span::styled(form.display_value_of(field).to_string(), value_style),
    ];
    if focused {
        let marker = if field == ConfigField::Language { "◀ ▶" } else { "▏" };
        spans.push(Span::styled(marker, theme.style_for_color(theme.accent_primary)));
    }
    Line::from(spans)
}

fn render_hint(frame: &mut Frame, area: Rect, form: &ConfigForm, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(form.field().hint(form.language)).style(theme.label_style()),
        area,
    );
}

fn render_error(frame: &mut Frame, area: Rect, form: &ConfigForm, theme: &Theme) {
    if let Some(err) = &form.error {
        frame.render_widget(
            Paragraph::new(err.clone()).style(theme.style_for(crate::theme::Status::Error)),
            area,
        );
    }
}

fn render_keys(frame: &mut Frame, area: Rect, theme: &Theme, lang: crate::i18n::Lang) {
    let line = Line::from(vec![
        Span::styled(" ↑↓ ", theme.plate_style(theme.accent_secondary)),
        Span::styled(i18n::t(lang, Key::Field), theme.label_style()),
        Span::styled(" 0-9 ", theme.plate_style(theme.accent_secondary)),
        Span::styled(i18n::t(lang, Key::Edit), theme.label_style()),
        Span::styled(" ENTER ", theme.plate_style(theme.ok)),
        Span::styled(i18n::t(lang, Key::Save), theme.label_style()),
        Span::styled(" ESC ", theme.plate_style(theme.error)),
        Span::styled(i18n::t(lang, Key::Cancel), theme.label_style()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
