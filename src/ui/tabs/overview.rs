use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::lxc::InstanceStatus;
use crate::ui::widgets::{gauge, panel};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let accent = theme.tab_accent(0);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_totals(frame, rows[0], app, accent);

    let host_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 3); 3])
        .split(rows[1]);

    gauge::render(
        frame,
        host_cols[0],
        theme,
        "cpu host",
        "",
        app.host.cpu_pct,
    );
    gauge::render(
        frame,
        host_cols[1],
        theme,
        "ram host",
        &format!("{:.1}/{:.1} GiB", app.host.mem_used_gb, app.host.mem_total_gb),
        app.host.mem_pct,
    );
    gauge::render(
        frame,
        host_cols[2],
        theme,
        "disco host",
        &format!("{:.1}/{:.1} GiB", app.host.disk_used_gb, app.host.disk_total_gb),
        app.host.disk_pct,
    );

    render_inventory(frame, rows[2], app, accent);
}

fn render_totals(frame: &mut Frame, area: Rect, app: &App, accent: ratatui::style::Color) {
    let theme = &app.theme;
    let block = panel::block(theme, i18n::t(app.lang, Key::OverviewStatusPanel), accent, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let running = app.instances.iter().filter(|i| i.status == InstanceStatus::Running).count();
    let stopped = app.instances.iter().filter(|i| i.status == InstanceStatus::Stopped).count();
    let frozen = app.instances.iter().filter(|i| i.status == InstanceStatus::Frozen).count();
    let errored = app.instances.iter().filter(|i| i.status == InstanceStatus::Error).count();

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 4); 4])
        .split(inner);

    stat(frame, cols[0], theme, "RUNNING", running, theme.ok);
    stat(frame, cols[1], theme, "STOPPED", stopped, theme.fg_muted);
    stat(frame, cols[2], theme, "FROZEN", frozen, theme.info);
    stat(frame, cols[3], theme, "ERROR", errored, theme.error);
}

fn stat(
    frame: &mut Frame,
    area: Rect,
    theme: &crate::theme::Theme,
    label: &str,
    value: usize,
    color: ratatui::style::Color,
) {
    let lines = vec![
        Line::from(Span::styled(format!("{value}"), theme.plate_style(color))),
        Line::from(Span::styled(label, theme.label_style())),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn storage_summary(app: &App) -> String {
    if app.inventory.pools.is_empty() {
        return i18n::t(app.lang, Key::OverviewNoPools).to_string();
    }
    let template = i18n::t(app.lang, Key::OverviewPoolTemplate);
    app.inventory
        .pools
        .iter()
        .map(|p| {
            template
                .replacen("{}", &p.name, 1)
                .replacen("{}", &p.driver, 1)
                .replacen("{}", &p.used_by.len().to_string(), 1)
        })
        .collect::<Vec<_>>()
        .join("  ·  ")
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3600;
    let minutes = (secs % 3600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn render_inventory(frame: &mut Frame, area: Rect, app: &App, accent: ratatui::style::Color) {
    let theme = &app.theme;
    let lang = app.lang;
    let block = panel::block(theme, i18n::t(lang, Key::OverviewInventoryPanel), accent, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let server_line = match &app.inventory.server {
        Some(s) => format!(
            "api {} · kernel {} · storage {}",
            s.api_version, s.environment.kernel, s.environment.storage
        ),
        None => i18n::t(lang, Key::OverviewNoServerData).to_string(),
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(i18n::t(lang, Key::OverviewLabelServer), theme.label_style()),
            Span::styled(server_line, theme.value_style()),
        ]),
        Line::from(vec![
            Span::styled("STORAGE    ", theme.label_style()),
            Span::styled(storage_summary(app), theme.readout_style()),
        ]),
        Line::from(vec![
            Span::styled(i18n::t(lang, Key::OverviewLabelNetworks), theme.label_style()),
            Span::styled(format!("{} definida(s)", app.inventory.networks.len()), theme.readout_style()),
        ]),
        Line::from(vec![
            Span::styled(i18n::t(lang, Key::OverviewLabelImages), theme.label_style()),
            Span::styled(format!("{} local(es)", app.inventory.images.len()), theme.readout_style()),
        ]),
        Line::from(vec![
            Span::styled("UPTIME     ", theme.label_style()),
            Span::styled(format_uptime(app.host.uptime_secs), theme.readout_style()),
        ]),
        Line::from(vec![
            Span::styled(i18n::t(lang, Key::OverviewLabelSystem), theme.label_style()),
            Span::styled(
                if app.system_check.report.all_blocking_ok() {
                    i18n::t(lang, Key::OverviewChecksOk).to_string()
                } else {
                    i18n::t(lang, Key::OverviewChecksPending).to_string()
                },
                theme.style_for(if app.system_check.report.all_blocking_ok() {
                    crate::theme::Status::Ok
                } else {
                    crate::theme::Status::Error
                }),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}
