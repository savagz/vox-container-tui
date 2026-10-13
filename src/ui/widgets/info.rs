use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Clear, Paragraph, Wrap},
};

use crate::app::App;
use crate::i18n::{self, Key};

use super::panel;

/// Popup de `lxc info <name>` + `lxc config show <name>` (pestaña CONTAINERS,
/// tecla `i`): salida cruda de ambos comandos, de sólo lectura y scrolleable
/// (↑↓, Pg Up/Dn, Home/End — ver `App::on_info_key`). `R` re-pide ambos
/// comandos, cualquier otra tecla cierra el popup.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;

    let modal = panel::centered(area, 76, 80);
    frame.render_widget(Clear, modal);

    let name = app.info_target.as_deref().unwrap_or("—");
    let title = i18n::t(lang, Key::InfoTitleTemplate).replacen("{}", name, 1);
    let block = panel::block(theme, &title, theme.accent_primary, true)
        .title_bottom(panel::readout(theme, i18n::t(lang, Key::InfoHint), theme.fg_muted));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let text = if app.info_loading {
        i18n::t(lang, Key::InfoLoading).to_string()
    } else {
        app.info_text.clone().unwrap_or_default()
    };

    // El scroll guardado en `App` no conoce el ancho real (no sabe cuánto
    // wrappea cada línea larga) ni el alto visible de este marco en particular
    // — acá se lo acota contra las líneas crudas menos lo que entra en pantalla,
    // así "End" no deja el texto vacío ni el scroll se pasa de largo.
    let line_count = text.lines().count() as u16;
    let max_scroll = line_count.saturating_sub(inner.height);
    let scroll = app.info_scroll.min(max_scroll);

    frame.render_widget(
        Paragraph::new(text).style(theme.value_style()).wrap(Wrap { trim: false }).scroll((scroll, 0)),
        inner,
    );
}
