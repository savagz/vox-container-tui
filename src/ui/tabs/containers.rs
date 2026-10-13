use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Row, Table},
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::lxc::InstanceStatus;
use crate::theme::{Status, glyph};
use crate::ui::format;
use crate::ui::widgets::panel;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let accent = theme.tab_accent(1);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    render_filter_line(frame, rows[0], app);
    render_table(frame, rows[1], app, accent);
}

fn render_filter_line(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    if !app.filter_active && app.filter_input.is_empty() {
        return;
    }
    let line = Line::from(vec![
        Span::styled(i18n::t(app.lang, Key::ContainersFilterLabel), theme.plate_style(theme.accent_secondary)),
        Span::styled(format!(" {}", app.filter_input), theme.value_style()),
        if app.filter_active {
            Span::styled("▏", theme.style_for_color(theme.accent_secondary))
        } else {
            Span::raw("")
        },
    ]);
    frame.render_widget(ratatui::widgets::Paragraph::new(line), area);
}

fn render_table(frame: &mut Frame, area: Rect, app: &App, accent: ratatui::style::Color) {
    let theme = &app.theme;
    let instances = app.filtered_instances();

    let detail = match app.selected_instance() {
        Some(inst) => i18n::t(app.lang, Key::ContainersDetailWithSelectionTemplate)
            .replacen("{}", &instances.len().to_string(), 1)
            .replacen("{}", &inst.architecture, 1)
            .replacen("{}", &inst.created_at, 1)
            .replacen("{}", &inst.profiles.join(","), 1),
        None => i18n::t(app.lang, Key::ContainersDetailTemplate).replacen("{}", &instances.len().to_string(), 1),
    };
    let block = panel::block(theme, super::Tab::Containers.title(app.lang), accent, false).title_bottom(panel::readout(
        theme,
        &detail,
        theme.fg_muted,
    ));

    let header = Row::new([
        "",
        i18n::t(app.lang, Key::ColName),
        i18n::t(app.lang, Key::ColStatus),
        i18n::t(app.lang, Key::ColType),
        "IPV4",
        "CPU%",
        "MEM",
        "SNAP",
    ])
    .style(theme.table_header_style());

    let selected = app.containers_row;
    let rows = instances.iter().enumerate().map(|(i, inst)| {
        let status = status_for(inst.status);
        let led = if inst.status == InstanceStatus::Running {
            glyph::LED_ON
        } else {
            glyph::LED_OFF
        };
        let cpu = inst
            .cpu_pct
            .map(|p| format!("{p:>5.1}"))
            .unwrap_or_else(|| "  n/a".to_string());
        let mem = format::bytes(inst.memory_bytes);
        let ip = inst.ipv4.first().cloned().unwrap_or_else(|| "—".to_string());

        let row = Row::new([
            Span::styled(led, theme.style_for(status)),
            Span::styled(inst.name.clone(), theme.style_for_bold()),
            Span::styled(inst.status.localized(app.lang), theme.style_for(status)),
            Span::styled(inst.kind.clone(), theme.label_style()),
            Span::styled(ip, theme.readout_style()),
            Span::styled(cpu, theme.readout_style()),
            Span::styled(mem, theme.readout_style()),
            Span::styled(inst.snapshots.len().to_string(), theme.label_style()),
        ]);

        if i == selected {
            row.style(theme.selected_row_style())
        } else {
            row
        }
    });

    let widths = [
        Constraint::Length(1),
        Constraint::Percentage(20),
        Constraint::Length(11),
        Constraint::Length(10),
        Constraint::Percentage(18),
        Constraint::Length(6),
        Constraint::Length(9),
        Constraint::Length(5),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1).block(block);
    frame.render_widget(table, area);
}

fn status_for(status: InstanceStatus) -> Status {
    match status {
        InstanceStatus::Running => Status::Ok,
        InstanceStatus::Frozen | InstanceStatus::Starting | InstanceStatus::Stopping => {
            Status::Warning
        }
        InstanceStatus::Error => Status::Error,
        InstanceStatus::Stopped | InstanceStatus::Unknown => Status::Info,
    }
}
