use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Row, Table, Wrap},
};

use crate::app::ImageCatalogState;
use crate::i18n::{self, Key, Lang};
use crate::lxc;
use crate::theme::Theme;
use crate::ui::format;

use super::panel;

/// Modal de descarga de imágenes (pestaña IMAGES, tecla `W`): catálogo
/// combinado de `DOWNLOAD_REMOTES`, filtro propio (`/`, matchea también por
/// remote) y descarga (`Enter`) de la fila seleccionada. Se abre encima de la
/// tabla de imágenes locales sin taparla del todo — decisión tomada con el
/// usuario, en vez de una vista alterna dentro de la misma pestaña.
pub fn render(frame: &mut Frame, area: Rect, state: &ImageCatalogState, theme: &Theme, lang: Lang) {
    let modal = panel::centered(area, 84, 74);
    frame.render_widget(Clear, modal);

    let accent = theme.accent_senary;
    let detail = if state.loading {
        i18n::t(lang, Key::ImageCatalogLoading).to_string()
    } else if let Some(err) = &state.error {
        i18n::t(lang, Key::ImageCatalogErrorTemplate).replacen("{}", err, 1)
    } else {
        let remotes = lxc::DOWNLOAD_REMOTES.join(", ");
        i18n::t(lang, Key::ImageCatalogDetailTemplate)
            .replacen("{}", &state.filtered().len().to_string(), 1)
            .replacen("{}", &state.catalog.len().to_string(), 1)
            .replacen("{}", &remotes, 1)
    };
    let detail_color = if state.error.is_some() { theme.error } else { theme.fg_muted };

    let block = panel::block(theme, i18n::t(lang, Key::ImageCatalogTitle), accent, true)
        .title_bottom(panel::readout(theme, &detail, detail_color));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    render_filter_line(frame, rows[0], state, theme, lang);
    render_body(frame, rows[1], state, theme, lang);
    render_keys(frame, rows[2], theme, lang);
}

fn render_filter_line(frame: &mut Frame, area: Rect, state: &ImageCatalogState, theme: &Theme, lang: Lang) {
    if !state.filter_active && state.filter.is_empty() {
        return;
    }
    let line = Line::from(vec![
        Span::styled(i18n::t(lang, Key::ContainersFilterLabel), theme.plate_style(theme.accent_secondary)),
        Span::styled(format!(" {}", state.filter), theme.value_style()),
        if state.filter_active {
            Span::styled("▏", theme.style_for_color(theme.accent_secondary))
        } else {
            Span::raw("")
        },
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_body(frame: &mut Frame, area: Rect, state: &ImageCatalogState, theme: &Theme, lang: Lang) {
    let catalog = state.filtered();

    if state.loading && state.error.is_none() && state.catalog.is_empty() {
        frame.render_widget(
            Paragraph::new(i18n::t(lang, Key::ImageCatalogLoading)).style(theme.label_style()),
            area,
        );
        return;
    }

    if let Some(err) = &state.error {
        frame.render_widget(
            Paragraph::new(i18n::t(lang, Key::ImageCatalogErrorTemplate).replacen("{}", err, 1))
                .style(theme.style_for(crate::theme::Status::Error))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    if catalog.is_empty() {
        frame.render_widget(
            Paragraph::new(i18n::t(lang, Key::ImageCatalogEmpty)).style(theme.label_style()),
            area,
        );
        return;
    }

    let header = Row::new([
        "REMOTE",
        "ALIAS",
        i18n::t(lang, Key::ImagesColDescription),
        "ARCH",
        i18n::t(lang, Key::ImagesColSize),
        i18n::t(lang, Key::ImagesColUploaded),
    ])
    .style(theme.table_header_style());

    let selected = state.row.min(catalog.len().saturating_sub(1));

    // La tabla no es "stateful" (no hay `TableState` persistido entre frames,
    // como en el resto de las tablas de la app): sin ventana manual, con más
    // filas que alto, `Table` siempre pinta desde la fila 0 y la selección se
    // pierde de vista al bajar. Se recalcula la ventana en cada render a partir
    // de la fila seleccionada — igual de barato, sin estado extra que sincronizar.
    let content_height = area.height.saturating_sub(1) as usize; // -1: fila de encabezado
    let len = catalog.len();
    let start = if content_height == 0 || len <= content_height {
        0
    } else {
        selected.saturating_sub(content_height - 1).min(len - content_height)
    };
    let end = (start + content_height.max(1)).min(len);

    let rows = catalog[start..end].iter().enumerate().map(|(i, entry)| {
        let img = &entry.image;
        let alias = img
            .aliases
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| i18n::t(lang, Key::ImagesNoAlias).to_string());
        let description = img.description().unwrap_or_else(|| i18n::t(lang, Key::ImagesNoDescription));

        let row = Row::new([
            Span::styled(entry.remote, theme.label_style()),
            Span::styled(alias, theme.style_for_bold()),
            Span::styled(description.to_string(), theme.value_style()),
            Span::styled(img.architecture.clone(), theme.label_style()),
            Span::styled(format::bytes(img.size), theme.readout_style()),
            Span::styled(img.uploaded_at.clone(), theme.label_style()),
        ]);
        if start + i == selected {
            row.style(theme.selected_row_style())
        } else {
            row
        }
    });

    let widths = [
        Constraint::Length(12),
        Constraint::Percentage(18),
        Constraint::Percentage(34),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Percentage(16),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1);
    frame.render_widget(table, area);
}

fn render_keys(frame: &mut Frame, area: Rect, theme: &Theme, lang: Lang) {
    let line = Line::from(vec![
        Span::styled(" ENTER ", theme.plate_style(theme.ok)),
        Span::styled(i18n::t(lang, Key::HbDownload), theme.label_style()),
        Span::styled(" / ", theme.plate_style(theme.accent_secondary)),
        Span::styled(i18n::t(lang, Key::HbFilter), theme.label_style()),
        Span::styled(" R ", theme.plate_style(theme.accent_secondary)),
        Span::styled(i18n::t(lang, Key::HbRefresh), theme.label_style()),
        Span::styled(" ESC ", theme.plate_style(theme.accent_tertiary)),
        Span::styled(i18n::t(lang, Key::Cancel), theme.label_style()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
