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
    let accent = theme.tab_accent(4);

    let instance_name = app.selected_instance_name.clone().unwrap_or_else(|| "—".to_string());
    let block = panel::block(theme, "snapshots", accent, false).title_bottom(panel::readout(
        theme,
        &i18n::t(lang, Key::SnapshotsInstanceTemplate).replacen("{}", &instance_name, 1),
        theme.fg_muted,
    ));

    let Some(instance) = app.selected_instance() else {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            ratatui::widgets::Paragraph::new(i18n::t(lang, Key::SnapshotsNoneSelected)).style(theme.label_style()),
            inner,
        );
        return;
    };

    let header = Row::new([i18n::t(lang, Key::ColName), i18n::t(lang, Key::ColCreated), "STATEFUL"])
        .style(theme.table_header_style());
    let selected = app.snapshot_row;

    let rows = instance.snapshots.iter().enumerate().map(|(i, snap)| {
        let row = Row::new([
            Span::styled(snap.name.clone(), theme.style_for_bold()),
            Span::styled(snap.created_at.clone(), theme.readout_style()),
            Span::styled(
                if snap.stateful { i18n::t(lang, Key::Yes) } else { "no" },
                theme.label_style(),
            ),
        ]);
        if i == selected {
            row.style(theme.selected_row_style())
        } else {
            row
        }
    });

    let widths = [
        Constraint::Percentage(40),
        Constraint::Percentage(40),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1).block(block);
    frame.render_widget(table, area);
}
