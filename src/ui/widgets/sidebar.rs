use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::lxc::InstanceStatus;
use crate::theme::glyph;
use crate::ui::tabs::Tab;

/// Columna izquierda: botonera del tablero. Cada sección es un pulsador con su
/// LED de identidad; el activo queda "hundido" (fondo en relieve y marco propio).
///
/// El LED tiene dos capas: el COLOR es la identidad fija de la sección (tabla del
/// plan), el GLIFO marca actividad (◉ si está activa o tiene datos vivos). Una
/// condición de alarma en esa sección (una instancia en ERROR, un chequeo
/// bloqueante en rojo) pisa la identidad y pone el LED en rojo con ◉ — así un
/// vistazo a la columna izquierda dice dónde está el problema sin navegar.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let mut constraints = Vec::with_capacity(Tab::ALL.len() + 1);
    for _ in Tab::ALL {
        constraints.push(Constraint::Length(3));
    }
    constraints.push(Constraint::Min(0));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for tab in Tab::ALL {
        render_button(frame, rows[tab.index()], tab, app);
    }

    render_legend(frame, rows[Tab::ALL.len()], app);
}

fn has_alarm(tab: Tab, app: &App) -> bool {
    match tab {
        Tab::Containers => app
            .instances
            .iter()
            .any(|i| i.status == InstanceStatus::Error),
        Tab::SystemCheck => !app.system_check.report.all_blocking_ok(),
        _ => false,
    }
}

fn is_live(tab: Tab, app: &App) -> bool {
    match tab {
        Tab::Overview => true,
        Tab::Containers => !app.instances.is_empty(),
        Tab::Resources => !app.instances.is_empty(),
        Tab::Network => !app.inventory.networks.is_empty(),
        Tab::Snapshots => app.selected_instance().is_some_and(|i| !i.snapshots.is_empty()),
        Tab::Images => !app.inventory.images.is_empty(),
        Tab::Profiles => !app.inventory.profiles.is_empty(),
        Tab::Logs => app.log_text.is_some(),
        Tab::Storage => !app.inventory.pools.is_empty(),
        Tab::SystemCheck => true,
    }
}

fn render_button(frame: &mut Frame, area: Rect, tab: Tab, app: &App) {
    let theme = &app.theme;
    let selected = tab == app.active_tab;
    let alarm = has_alarm(tab, app);
    let led_color = if alarm { theme.error } else { theme.tab_accent(tab.index()) };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if selected { BorderType::Thick } else { BorderType::Plain })
        .border_style(theme.border_style(selected))
        .style(if selected { theme.raised_style() } else { theme.panel_style() });

    let label_style = if selected {
        theme.style_for_bold()
    } else {
        theme.tab_inactive_style()
    };

    let glyph = if alarm || selected || is_live(tab, app) {
        glyph::LED_ON
    } else {
        glyph::LED_OFF
    };

    let line = Line::from(vec![
        Span::styled(format!(" {glyph} "), theme.style_for_color(led_color)),
        Span::styled(tab.title().to_uppercase(), label_style),
    ]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}

/// Pie de la botonera: recordatorio de atajos siempre visible.
fn render_legend(frame: &mut Frame, area: Rect, app: &App) {
    if area.height < 3 {
        return;
    }
    let theme = &app.theme;

    let backend_label = if app.backend.fixtures {
        "FIXTURES"
    } else {
        match app.backend.cli {
            Some(c) => match c {
                crate::lxc::cli::Cli::Incus => "INCUS",
                crate::lxc::cli::Cli::Lxc => "LXD",
            },
            None => i18n::t(app.lang, Key::SidebarNoBackend),
        }
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(" [C] ", theme.plate_style(theme.accent_primary)),
            Span::styled(format!(" {}", i18n::t(app.lang, Key::SidebarConfig)), theme.label_style()),
        ]),
        Line::from(vec![
            Span::styled(" [H] ", theme.plate_style(theme.accent_secondary)),
            Span::styled(format!(" {}", i18n::t(app.lang, Key::SidebarHelp)), theme.label_style()),
        ]),
        Line::from(Span::styled(format!("     {backend_label}"), theme.label_style())),
    ];

    frame.render_widget(Paragraph::new(lines).style(theme.base_style()), area);
}
