use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Row, Table},
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::ui::widgets::panel;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let accent = theme.tab_accent(6);

    let block = panel::block(theme, "profiles", accent, false).title_bottom(panel::readout(
        theme,
        &i18n::t(lang, Key::ProfilesDetailTemplate).replacen("{}", &app.inventory.profiles.len().to_string(), 1),
        theme.fg_muted,
    ));

    let header = Row::new([
        i18n::t(lang, Key::ColName),
        i18n::t(lang, Key::ImagesColDescription),
        i18n::t(lang, Key::NetworkColUsedBy),
    ])
    .style(theme.table_header_style());

    let selected = app.profile_row;
    let rows = app.inventory.profiles.iter().enumerate().map(|(i, profile)| {
        let description = if profile.description.is_empty() {
            i18n::t(lang, Key::ImagesNoDescription).to_string()
        } else {
            profile.description.clone()
        };

        let row = Row::new([
            Span::styled(profile.name.clone(), theme.style_for_bold()),
            Span::styled(description, theme.value_style()),
            Span::styled(profile.used_by.len().to_string(), theme.readout_style()),
        ]);
        if i == selected {
            row.style(theme.selected_row_style())
        } else {
            row
        }
    });

    let widths = [
        Constraint::Percentage(24),
        Constraint::Percentage(56),
        Constraint::Percentage(20),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1).block(block);
    frame.render_widget(table, area);
}
