use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Row, Table},
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::ui::widgets::panel;

/// Pools (`lxc storage list`) como entrada, con drill-down (`Enter`) a los
/// volúmenes de uno (`app.storage_drill`) — mismo patrón que Snapshots
/// (instancia seleccionada), pero acá el drill vive dentro de la propia
/// pestaña en vez de cruzar a otra.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    match &app.storage_drill {
        Some(pool) => render_volumes(frame, area, app, pool),
        None => render_pools(frame, area, app),
    }
}

fn render_pools(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let accent = theme.tab_accent(7);

    let detail =
        i18n::t(lang, Key::StorageDetailTemplate).replacen("{}", &app.inventory.pools.len().to_string(), 1);
    let block =
        panel::block(theme, super::Tab::Storage.title(app.lang), accent, false).title_bottom(panel::readout(theme, &detail, theme.fg_muted));

    let header = Row::new([i18n::t(lang, Key::ColName), i18n::t(lang, Key::ColDriver), i18n::t(lang, Key::NetworkColUsedBy)])
        .style(theme.table_header_style());

    let selected = app.storage_row;
    let rows = app.inventory.pools.iter().enumerate().map(|(i, pool)| {
        let row = Row::new([
            Span::styled(pool.name.clone(), theme.style_for_bold()),
            Span::styled(pool.driver.clone(), theme.value_style()),
            Span::styled(pool.used_by.len().to_string(), theme.readout_style()),
        ]);
        if i == selected {
            row.style(theme.selected_row_style())
        } else {
            row
        }
    });

    let widths = [Constraint::Percentage(36), Constraint::Percentage(32), Constraint::Percentage(32)];
    let table = Table::new(rows, widths).header(header).column_spacing(1).block(block);
    frame.render_widget(table, area);
}

fn render_volumes(frame: &mut Frame, area: Rect, app: &App, pool: &str) {
    let theme = &app.theme;
    let lang = app.lang;
    let accent = theme.tab_accent(7);

    let volumes: Vec<_> = app.inventory.volumes.iter().filter(|v| v.pool == pool).collect();
    let detail = i18n::t(lang, Key::StorageVolumesOfPoolTemplate)
        .replacen("{}", &volumes.len().to_string(), 1)
        .replacen("{}", pool, 1);
    let block =
        panel::block(theme, super::Tab::Storage.title(app.lang), accent, false).title_bottom(panel::readout(theme, &detail, theme.fg_muted));

    let header = Row::new([
        i18n::t(lang, Key::ColName),
        i18n::t(lang, Key::ColType),
        i18n::t(lang, Key::ColContentType),
        i18n::t(lang, Key::NetworkColUsedBy),
    ])
    .style(theme.table_header_style());

    let selected = app.storage_volume_row;
    let rows = volumes.iter().enumerate().map(|(i, vol)| {
        let row = Row::new([
            Span::styled(vol.name.clone(), theme.style_for_bold()),
            Span::styled(vol.kind.clone(), theme.value_style()),
            Span::styled(vol.content_type.clone(), theme.value_style()),
            Span::styled(vol.used_by.len().to_string(), theme.readout_style()),
        ]);
        if i == selected {
            row.style(theme.selected_row_style())
        } else {
            row
        }
    });

    let widths = [
        Constraint::Percentage(32),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(28),
    ];
    let table = Table::new(rows, widths).header(header).column_spacing(1).block(block);
    frame.render_widget(table, area);
}
