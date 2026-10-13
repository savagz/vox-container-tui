use chrono::Local;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::lxc::InstanceStatus;
use crate::theme::{Status, glyph};

use super::panel;

/// Franja superior: identificación del backend a la izquierda, pilotos de estado
/// (daemon, contadores por estado, reloj) a la derecha — como un frente de rack.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;

    let block = panel::block(
        theme,
        i18n::t(app.lang, Key::HeaderTitle),
        theme.accent_primary,
        true,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(inner);

    let server_version = app
        .inventory
        .server
        .as_ref()
        .map(|s| s.environment.server_version.as_str())
        .unwrap_or("—");

    let target = Line::from(vec![
        Span::styled("BACKEND ", theme.label_style()),
        Span::styled(backend_label(app), theme.readout_style()),
        Span::styled(format!("  ·  {server_version}"), theme.label_style()),
    ]);
    frame.render_widget(Paragraph::new(target), cols[0]);

    let running = app
        .instances
        .iter()
        .filter(|i| i.status == InstanceStatus::Running)
        .count();
    let stopped = app
        .instances
        .iter()
        .filter(|i| i.status == InstanceStatus::Stopped)
        .count();
    let errored = app
        .instances
        .iter()
        .filter(|i| i.status == InstanceStatus::Error)
        .count();

    let daemon_status = if app.lxc_error.is_some() {
        Status::Error
    } else if app.backend.fixtures {
        Status::Info
    } else {
        Status::Ok
    };

    let pilots = Line::from(vec![
        led(theme, daemon_status),
        Span::styled(
            format!("{}  ", daemon_label(app)),
            theme.style_for(daemon_status),
        ),
        Span::styled(format!("{running} UP "), theme.style_for(Status::Ok)),
        Span::styled(format!("{stopped} DOWN "), theme.label_style()),
        if errored > 0 {
            Span::styled(format!("{errored} ERR  "), theme.style_for(Status::Error))
        } else {
            Span::styled("", theme.label_style())
        },
        Span::styled(Local::now().format("%H:%M:%S").to_string(), theme.readout_style()),
    ]);
    frame.render_widget(Paragraph::new(pilots).alignment(Alignment::Right), cols[1]);
}

fn led<'a>(theme: &crate::theme::Theme, status: Status) -> Span<'a> {
    let glyph = if status == Status::Error {
        glyph::LED_OFF
    } else {
        glyph::LED_ON
    };
    Span::styled(format!("{glyph} "), theme.style_for(status))
}

fn backend_label(app: &App) -> String {
    if app.backend.fixtures {
        return "FIXTURES".to_string();
    }
    match app.backend.cli {
        Some(crate::lxc::cli::Cli::Incus) => "INCUS".to_string(),
        Some(crate::lxc::cli::Cli::Lxc) => "LXD".to_string(),
        None => i18n::t(app.lang, Key::HeaderBackendNotInstalled).to_string(),
    }
}

/// Sólo el estado, corto — el texto completo del error va en el banner de
/// ancho completo debajo del header (`ui::draw`), no acá: 28 caracteres no
/// alcanzan para un mensaje real y cortarlo a la mitad lo vuelve ilegible.
fn daemon_label(app: &App) -> String {
    if app.lxc_error.is_some() {
        format!("{} DAEMON: ERROR", glyph::BOLT)
    } else if app.backend.fixtures {
        "DAEMON: FIXTURES".to_string()
    } else {
        "DAEMON: OK".to_string()
    }
}
