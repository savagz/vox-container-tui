use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::theme::segmented_bar;
use crate::ui::format;
use crate::ui::widgets::panel;

const BAR_WIDTH: usize = 14;

/// Uso de recursos por instancia: barras segmentadas de CPU/MEM y disco total,
/// mismo lenguaje visual que los gauges del host pero una fila por contenedor.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let accent = theme.tab_accent(2);

    let block = panel::block(theme, "resources", accent, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.instances.is_empty() {
        frame.render_widget(
            Paragraph::new(i18n::t(app.lang, Key::ResourcesEmpty)).style(theme.label_style()),
            inner,
        );
        return;
    }

    let lines: Vec<Line> = app
        .instances
        .iter()
        .map(|inst| {
            let cpu_pct = inst.cpu_pct.unwrap_or(0.0).clamp(0.0, 100.0);
            let (cpu_fill, cpu_empty) = segmented_bar(cpu_pct, BAR_WIDTH);
            let cpu_color = theme.color_for(theme.status_for_pct(cpu_pct));

            let mem_limit_bytes = parse_memory_limit(inst.limits_memory.as_deref());
            let mem_pct = mem_limit_bytes
                .filter(|&l| l > 0)
                .map(|l| (inst.memory_bytes as f64 / l as f64 * 100.0).clamp(0.0, 100.0))
                .unwrap_or(0.0);
            let (mem_fill, mem_empty) = segmented_bar(mem_pct, BAR_WIDTH);
            let mem_color = theme.color_for(theme.status_for_pct(mem_pct));

            Line::from(vec![
                Span::styled(format!("{:<16}", truncate(&inst.name, 16)), theme.style_for_bold()),
                Span::styled(" CPU ", theme.label_style()),
                Span::styled(cpu_fill, theme.style_for_color(cpu_color)),
                Span::styled(cpu_empty, theme.style_for_color(theme.fg_dim)),
                Span::styled(format!(" {cpu_pct:>5.1}% "), theme.readout_style()),
                Span::styled(" MEM ", theme.label_style()),
                Span::styled(mem_fill, theme.style_for_color(mem_color)),
                Span::styled(mem_empty, theme.style_for_color(theme.fg_dim)),
                Span::styled(
                    format!(
                        " {}/{} ",
                        format::bytes(inst.memory_bytes),
                        inst.limits_memory.clone().unwrap_or_else(|| "—".to_string())
                    ),
                    theme.readout_style(),
                ),
                Span::styled(" DISK ", theme.label_style()),
                Span::styled(format::bytes(inst.disk_bytes), theme.readout_style()),
                Span::styled(" NET ", theme.label_style()),
                Span::styled(
                    format!("↓{} ↑{}", format::bytes(inst.net_rx_bytes), format::bytes(inst.net_tx_bytes)),
                    theme.readout_style(),
                ),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

/// `limits.memory` acepta sufijos LXD (`MiB`/`GiB`/`kB`/...); sólo se soportan
/// los dos más comunes — alcanza para pintar la barra, no hace falta el parser
/// completo del daemon.
fn parse_memory_limit(raw: Option<&str>) -> Option<u64> {
    let raw = raw?.trim();
    if let Some(v) = raw.strip_suffix("GiB") {
        return v.trim().parse::<f64>().ok().map(|g| (g * 1024.0 * 1024.0 * 1024.0) as u64);
    }
    if let Some(v) = raw.strip_suffix("MiB") {
        return v.trim().parse::<f64>().ok().map(|m| (m * 1024.0 * 1024.0) as u64);
    }
    raw.parse::<u64>().ok()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}
