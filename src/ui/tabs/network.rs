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
    let accent = theme.tab_accent(3);

    let block = panel::block(theme, super::Tab::Network.title(app.lang), accent, false).title_bottom(panel::readout(
        theme,
        &i18n::t(lang, Key::NetworkDetailTemplate).replacen("{}", &app.inventory.networks.len().to_string(), 1),
        theme.fg_muted,
    ));

    let header = Row::new([
        i18n::t(lang, Key::ColName),
        i18n::t(lang, Key::ColType),
        i18n::t(lang, Key::NetworkColManaged),
        "IPV4",
        i18n::t(lang, Key::NetworkColUsedBy),
    ])
    .style(theme.table_header_style());

    let selected = app.network_row;
    let rows = app.inventory.networks.iter().enumerate().map(|(i, net)| {
        let managed_color = if net.managed { theme.ok } else { theme.fg_muted };
        let ipv4 = net
            .config
            .get("ipv4.address")
            .cloned()
            .unwrap_or_else(|| "—".to_string());
        let row = Row::new([
            Span::styled(net.name.clone(), theme.style_for_bold()),
            Span::styled(net.kind.clone(), theme.label_style()),
            Span::styled(
                if net.managed { i18n::t(lang, Key::Yes) } else { "no" },
                theme.style_for_color(managed_color),
            ),
            Span::styled(ipv4, theme.readout_style()),
            Span::styled(net.used_by.len().to_string(), theme.label_style()),
        ]);
        if i == selected {
            row.style(theme.selected_row_style())
        } else {
            row
        }
    });

    let widths = [
        Constraint::Percentage(20),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Percentage(28),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1).block(block);
    frame.render_widget(table, area);
}
