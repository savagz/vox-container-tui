use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};

use crate::app::App;
use crate::i18n::{self, Key, Lang};
use crate::theme::Theme;

use super::panel;

/// Contenido lógico del modal, independiente de estilos — una sola fuente de
/// verdad para `render` (que lo pinta) y `line_count` (que lo mide para poder
/// acotar el scroll en `App::on_help_key`). Evita que ambos se desincronicen
/// si se agrega/saca una fila.
enum Entry {
    Section(&'static str),
    Row(&'static str, &'static str),
    Manual(&'static str),
    Blank,
    Close(&'static str),
}

fn entries(lang: Lang) -> Vec<Entry> {
    use Entry::*;
    vec![
        Section(i18n::t(lang, Key::HelpSectionGlobal)),
        Row("TAB / 1-9", i18n::t(lang, Key::HbJump)),
        Row("↑↓ / j k", i18n::t(lang, Key::HbNavigate)),
        Row("Home/End / g G", "top / bottom"),
        Row("R", i18n::t(lang, Key::HbRefresh)),
        Row("C", i18n::t(lang, Key::HbConfig)),
        Row("H", i18n::t(lang, Key::SidebarHelp)),
        Row("Q", i18n::t(lang, Key::HbQuit)),
        Blank,
        Section(i18n::t(lang, Key::HelpSectionContainers)),
        Row("/", i18n::t(lang, Key::HbFilter)),
        Row("S", "start"),
        Row("X", "stop"),
        Row("⇧R", "restart"),
        Row("P", "pause"),
        Row("N", i18n::t(lang, Key::HbNew)),
        Row("D", i18n::t(lang, Key::HbClone)),
        Row("M", i18n::t(lang, Key::HbRename)),
        Row("⇧S", "snapshot"),
        Row("E", i18n::t(lang, Key::HbConsole)),
        Row("L", i18n::t(lang, Key::HbLimits)),
        Row("O", i18n::t(lang, Key::HbConfigKeys)),
        Row("⇧P", "profiles"),
        Row("⇧U", i18n::t(lang, Key::HbUnset)),
        Row("F", i18n::t(lang, Key::HbFiles)),
        Row("T", i18n::t(lang, Key::HbDevices)),
        Row("⇧B", i18n::t(lang, Key::HbPublish)),
        Row("V", i18n::t(lang, Key::HbViewLogs)),
        Row("I", i18n::t(lang, Key::HbInfo)),
        Row("⇧E", i18n::t(lang, Key::HbExport)),
        Row("⇧I", i18n::t(lang, Key::HbImport)),
        Row("Supr", i18n::t(lang, Key::HbDelete)),
        Blank,
        Section(i18n::t(lang, Key::HelpSectionSnapshots)),
        Row("⇧S", i18n::t(lang, Key::HbNew)),
        Row("⇧R", i18n::t(lang, Key::HbRestore)),
        Row("Supr", i18n::t(lang, Key::HbDelete)),
        Blank,
        Section(i18n::t(lang, Key::HelpSectionImages)),
        Row("N", i18n::t(lang, Key::HbLaunch)),
        Row("A", "alias"),
        Row("W", i18n::t(lang, Key::HbDownload)),
        Row("I", i18n::t(lang, Key::HbInfo)),
        Row("⇧E", i18n::t(lang, Key::HbExport)),
        Row("⇧I", i18n::t(lang, Key::HbImport)),
        Row("Supr", i18n::t(lang, Key::HbDelete)),
        Blank,
        Section(i18n::t(lang, Key::HelpSectionProfiles)),
        Row("N", i18n::t(lang, Key::HbNew)),
        Row("⇧D", "description"),
        Row("L", i18n::t(lang, Key::HbLimits)),
        Row("I", i18n::t(lang, Key::HbInfo)),
        Row("Supr", i18n::t(lang, Key::HbDelete)),
        Blank,
        Section(i18n::t(lang, Key::HelpSectionLogs)),
        Row("R", i18n::t(lang, Key::HbRefresh)),
        Row("⇧O", i18n::t(lang, Key::HbOperations)),
        Row("⇧W", i18n::t(lang, Key::HbWarnings)),
        Blank,
        Section(i18n::t(lang, Key::HelpSectionNetwork)),
        Row("N", i18n::t(lang, Key::HbNew)),
        Row("B", i18n::t(lang, Key::HbAttach)),
        Row("O", i18n::t(lang, Key::HbConfigKeys)),
        Row("I", i18n::t(lang, Key::HbInfo)),
        Row("Supr", i18n::t(lang, Key::HbDelete)),
        Blank,
        Section(i18n::t(lang, Key::HelpSectionStorage)),
        Row("N", i18n::t(lang, Key::HbNew)),
        Row("Supr", i18n::t(lang, Key::HbDelete)),
        Row("I", i18n::t(lang, Key::HbInfo)),
        Row("Enter", i18n::t(lang, Key::HbEnter)),
        Manual(i18n::t(lang, Key::HelpStorageDrillHint)),
        Row("B", i18n::t(lang, Key::HbAttach)),
        Row("U", i18n::t(lang, Key::HbDetach)),
        Row("M", i18n::t(lang, Key::HbRename)),
        Row("O", i18n::t(lang, Key::HbConfigKeys)),
        Row("I", i18n::t(lang, Key::HbInfo)),
        Row("⇧S", "snapshot"),
        Row("⇧M", i18n::t(lang, Key::HbMoveTo)),
        Row("Esc", i18n::t(lang, Key::HbBack)),
        Blank,
        Section(i18n::t(lang, Key::HelpSectionSystemCheck)),
        Row("R", i18n::t(lang, Key::HbRecheck)),
        Row("Enter", i18n::t(lang, Key::HbFix)),
        Blank,
        // Comandos que no tienen atajo en el TUI todavía — referencia rápida
        // para correrlos a mano en una terminal aparte.
        Section(i18n::t(lang, Key::HelpSectionManual)),
        Manual(i18n::t(lang, Key::ManualBackup)),
        Manual(i18n::t(lang, Key::ManualNetwork)),
        Manual(i18n::t(lang, Key::ManualStorageVolume)),
        Blank,
        Close(i18n::t(lang, Key::HelpClose)),
    ]
}

/// Cuántas líneas lógicas tiene el modal en este idioma — cota para el scroll
/// en `App::on_help_key` (mismo criterio que `App::info_line_count`: no
/// considera wrapping, el widget se acota solo contra el alto visible al
/// dibujar).
pub fn line_count(lang: Lang) -> u16 {
    entries(lang).len() as u16
}

/// Referencia completa de atajos: modal de sólo lectura. ↑↓/PgUp/PgDn/Home/End
/// desplazan (`App::on_help_key`), cualquier otra tecla cierra. A diferencia de
/// la barra inferior (contextual, recortada al ancho disponible) acá entran
/// todas las secciones — el `Wrap` evita que corte texto a la mitad, el scroll
/// permite llegar a lo que no entra en pantallas chicas.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;

    let modal = panel::centered(area, 76, 86);
    frame.render_widget(Clear, modal);

    let block = panel::block(theme, i18n::t(lang, Key::HelpTitle), theme.accent_primary, true);
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let lines: Vec<Line> = entries(lang)
        .into_iter()
        .map(|entry| match entry {
            Entry::Section(text) => section(theme, text),
            Entry::Row(key, label) => row(theme, key, label),
            Entry::Manual(text) => manual_row(theme, text),
            Entry::Blank => Line::from(""),
            Entry::Close(text) => Line::from(Span::styled(text, theme.label_style())),
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((app.help_scroll, 0)),
        inner,
    );
}

fn section<'a>(theme: &Theme, text: &'a str) -> Line<'a> {
    Line::from(Span::styled(text, theme.style_for_bold()))
}

fn row<'a>(theme: &Theme, key: &'a str, label: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {key:<15}"), theme.style_for_color(theme.accent_secondary)),
        Span::styled(label, theme.label_style()),
    ])
}

/// Fila de la sección MANUAL: sin capuchón de tecla, sólo el comando — se
/// apoya en `Wrap` para partir en pantallas angostas.
fn manual_row<'a>(theme: &Theme, text: &'a str) -> Line<'a> {
    Line::from(Span::styled(format!("  {text}"), theme.label_style()))
}
