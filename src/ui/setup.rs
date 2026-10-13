use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use crate::deps::CheckState;
use crate::i18n::{self, Key};
use crate::setup::SystemCheckState;
use crate::theme::{Theme, glyph, hazard_stripe};

use super::widgets::panel;

/// Wizard a pantalla completa: corre antes que exista ningún tablero detrás.
pub fn render_gate(frame: &mut Frame, area: Rect, state: &SystemCheckState, theme: &Theme) {
    frame.render_widget(Block::default().style(theme.base_style()), area);
    render_frame(frame, area, state, theme, true);
}

/// Mismo cuerpo, embebido como pestaña del tablero (`SYSTEM CHECK`).
pub fn render_panel(frame: &mut Frame, area: Rect, state: &SystemCheckState, theme: &Theme) {
    render_frame(frame, area, state, theme, false);
}

fn render_frame(frame: &mut Frame, area: Rect, state: &SystemCheckState, theme: &Theme, gate: bool) {
    let lang = state.lang;
    let all_ok = state.report.all_blocking_ok();
    let accent = if all_ok { theme.ok } else { theme.error };
    let title = if gate {
        i18n::t(lang, Key::SetupGateTitle)
    } else {
        i18n::t(lang, Key::SetupPanelTitle)
    };

    let status_text = match (all_ok, state.report.package_manager, state.report.cli) {
        (true, _, Some(cli)) => i18n::t(lang, Key::SetupStatusAllOkWithBackendTemplate).replacen("{}", cli.binary(), 1),
        (true, _, None) => i18n::t(lang, Key::SetupStatusAllOk).to_string(),
        (false, Some(pm), _) => {
            i18n::t(lang, Key::SetupStatusActionRequiredWithPmTemplate).replacen("{}", pm.label(), 1)
        }
        (false, None, _) => i18n::t(lang, Key::SetupStatusActionRequired).to_string(),
    };
    let block = panel::block(theme, title, accent, true)
        .title_bottom(panel::readout(theme, &status_text, accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut constraints = vec![Constraint::Length(1)];
    constraints.push(Constraint::Min(3));
    constraints.push(Constraint::Length(3));
    constraints.push(Constraint::Length(1));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(constraints)
        .split(inner);

    render_stripe(frame, rows[0], theme, all_ok, gate, lang);
    render_checks(frame, rows[1], state, theme);
    render_detail(frame, rows[2], state, theme);
    render_keys(frame, rows[3], theme, gate, lang);
}

fn render_stripe(frame: &mut Frame, area: Rect, theme: &Theme, all_ok: bool, gate: bool, lang: crate::i18n::Lang) {
    let text = if all_ok {
        i18n::t(lang, Key::SetupStripeAllOk)
    } else if gate {
        i18n::t(lang, Key::SetupStripeGateBlocking)
    } else {
        i18n::t(lang, Key::SetupStripePanelBlocking)
    };
    let color = if all_ok { theme.ok } else { theme.warning };
    let stripe_width = area
        .width
        .saturating_sub(text.chars().count() as u16 + 2)
        .min(8) as usize;

    let line = Line::from(vec![
        Span::styled(hazard_stripe(stripe_width), theme.style_for_color(color)),
        Span::styled(format!(" {text} "), theme.style_for_color(color)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_checks(frame: &mut Frame, area: Rect, state: &SystemCheckState, theme: &Theme) {
    let lang = state.lang;
    let lines: Vec<Line> = state
        .report
        .checks
        .iter()
        .enumerate()
        .map(|(i, check)| {
            let selected = i == state.selected;
            let (led, status_color) = match &check.state {
                CheckState::Ok(_) => (glyph::LED_ON, theme.ok),
                CheckState::Missing { .. } if check.blocking => (glyph::LED_ON, theme.error),
                CheckState::Missing { .. } => (glyph::LED_ON, theme.warning),
                CheckState::Unknown(_) => (glyph::LED_OFF, theme.fg_muted),
            };

            let marker = if selected { "▌" } else { " " };
            let label_style = if selected {
                theme.style_for_bold()
            } else {
                theme.label_style()
            };

            let mut spans = vec![
                Span::styled(marker, theme.style_for_color(theme.accent_primary)),
                Span::styled(format!("{led} "), theme.style_for_color(status_color)),
                Span::styled(format!("{:<26}", check.label), label_style),
                Span::styled(check.state.detail().to_string(), theme.value_style()),
            ];
            if check.state.fix().is_some() {
                spans.push(Span::styled(i18n::t(lang, Key::SetupFixHint), theme.style_for_color(theme.accent_primary)));
            }
            Line::from(spans)
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_detail(frame: &mut Frame, area: Rect, state: &SystemCheckState, theme: &Theme) {
    let text = if let Some(notice) = &state.notice {
        notice.clone()
    } else if let Some(check) = state.report.get(state.selected) {
        match check.state.fix() {
            Some(fix) => i18n::t(state.lang, Key::SetupSuggestedCommandTemplate)
                .replacen("{}", &crate::deps::command_line(fix), 1),
            None => String::new(),
        }
    } else {
        String::new()
    };

    frame.render_widget(
        Paragraph::new(text).style(theme.readout_style()).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_keys(frame: &mut Frame, area: Rect, theme: &Theme, gate: bool, lang: crate::i18n::Lang) {
    let mut spans = vec![
        Span::styled(" ↑↓ ", theme.plate_style(theme.accent_secondary)),
        Span::styled(i18n::t(lang, Key::SetupKeyChoose), theme.label_style()),
        Span::styled(" ENTER ", theme.plate_style(theme.accent_primary)),
        Span::styled(i18n::t(lang, Key::SetupKeyFix), theme.label_style()),
        Span::styled(" R ", theme.plate_style(theme.accent_secondary)),
        Span::styled(i18n::t(lang, Key::SetupKeyRecheck), theme.label_style()),
    ];
    if gate {
        spans.push(Span::styled(" A ", theme.plate_style(theme.warning)));
        spans.push(Span::styled(i18n::t(lang, Key::SetupKeyContinueAnyway), theme.label_style()));
        spans.push(Span::styled(" ESC ", theme.plate_style(theme.error)));
        spans.push(Span::styled(i18n::t(lang, Key::SetupKeyExit), theme.label_style()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
