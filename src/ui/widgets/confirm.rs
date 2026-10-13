use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};

use crate::app::ConfirmState;
use crate::i18n::{self, Key, Lang};
use crate::theme::{Status, Theme, hazard_stripe};

use super::panel;

/// Modal de confirmación destructiva: franja de peligro + campo de texto que
/// exige tipear el nombre exacto de la instancia antes de habilitar `Enter`.
pub fn render(frame: &mut Frame, area: Rect, confirm: &ConfirmState, theme: &Theme, lang: Lang) {
    let modal = panel::centered(area, 64, 40);
    frame.render_widget(Clear, modal);

    let block = panel::block(theme, i18n::t(lang, Key::ConfirmTitle), theme.error, true);
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    let stripe_width = rows[0].width.min(64) as usize;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hazard_stripe(stripe_width),
            theme.style_for_color(theme.error),
        ))),
        rows[0],
    );

    frame.render_widget(
        Paragraph::new(confirm.message.clone())
            .style(theme.value_style())
            .wrap(Wrap { trim: true }),
        rows[1],
    );

    let expect_line = Line::from(vec![
        Span::styled(i18n::t(lang, Key::ConfirmExpectedName), theme.label_style()),
        Span::styled(confirm.expected.clone(), theme.readout_style()),
    ]);
    frame.render_widget(Paragraph::new(expect_line), rows[2]);

    let matches = confirm.input == confirm.expected;
    let input_style = if matches {
        theme.style_for(Status::Ok)
    } else {
        theme.selected_row_style()
    };
    let input_line = Line::from(vec![
        Span::styled("▌ ", theme.style_for_color(theme.error)),
        Span::styled(confirm.input.clone(), input_style),
        Span::styled("▏", theme.style_for_color(theme.error)),
    ]);
    frame.render_widget(Paragraph::new(input_line), rows[3]);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ENTER ", theme.plate_style(theme.error)),
            Span::styled(i18n::t(lang, Key::Confirm), theme.label_style()),
            Span::styled(" ESC ", theme.plate_style(theme.accent_tertiary)),
            Span::styled(i18n::t(lang, Key::Cancel), theme.label_style()),
        ])),
        rows[4],
    );
}
