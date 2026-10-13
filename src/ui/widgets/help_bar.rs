use ratatui::{
    Frame,
    layout::Rect,
    style::Color,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::theme::Theme;
use crate::ui::tabs::Tab;

/// Barra inferior: teclas grabadas como capuchones de tecla, contextuales según
/// la sección activa. Si hay un aviso reciente ocupa la barra hasta la próxima
/// tecla — igual que un mensaje de panel de control real.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;

    if let Some((message, status)) = &app.toast {
        let line = Line::from(vec![
            Span::styled(i18n::t(lang, Key::HbNotice), theme.plate_style(theme.color_for(*status))),
            Span::styled(format!(" {message}"), theme.style_for(*status)),
        ]);
        frame.render_widget(Paragraph::new(line).style(theme.base_style()), area);
        return;
    }

    let spans = build_spans(app);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).wrap(Wrap { trim: true }).style(theme.base_style()),
        area,
    );
}

/// Cuántas filas necesita la barra para no cortar atajos: mismo criterio que el
/// banner del daemon (`ui::banner_lines`) — contar caracteres y dividir por el
/// ancho disponible, sin medir con precisión Unicode porque todo el texto acá
/// es ASCII salvo un puñado de flechas de una sola columna.
pub fn height_for(app: &App, width: u16) -> u16 {
    if app.toast.is_some() {
        return 1;
    }
    let spans = build_spans(app);
    let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let usable = width.max(1) as usize;
    let lines = total.saturating_add(usable - 1) / usable;
    (lines.max(1) as u16).min(3)
}

fn build_spans(app: &App) -> Vec<Span<'static>> {
    let theme = &app.theme;
    let lang = app.lang;

    let mut spans = vec![
        key(theme, "TAB", theme.accent_secondary),
        label(theme, i18n::t(lang, Key::HbView)),
        key(theme, "1-9", theme.accent_secondary),
        label(theme, i18n::t(lang, Key::HbJump)),
        key(theme, "↑↓", theme.accent_secondary),
        label(theme, i18n::t(lang, Key::HbNavigate)),
    ];

    spans.extend(match app.active_tab {
        Tab::Containers => vec![
            key(theme, "S", theme.ok),
            label(theme, "start"),
            key(theme, "X", theme.warning),
            label(theme, "stop"),
            key(theme, "R", theme.accent_secondary),
            label(theme, "restart"),
            key(theme, "P", theme.accent_secondary),
            label(theme, "pause"),
            key(theme, "N", theme.accent_primary),
            label(theme, i18n::t(lang, Key::HbNew)),
            key(theme, "D", theme.accent_quinary),
            label(theme, i18n::t(lang, Key::HbClone)),
            key(theme, "M", theme.accent_quinary),
            label(theme, i18n::t(lang, Key::HbRename)),
            key(theme, "⇧S", theme.accent_quinary),
            label(theme, "snapshot"),
            key(theme, "E", theme.info),
            label(theme, i18n::t(lang, Key::HbConsole)),
            key(theme, "L", theme.accent_senary),
            label(theme, i18n::t(lang, Key::HbLimits)),
            key(theme, "O", theme.accent_senary),
            label(theme, i18n::t(lang, Key::HbConfigKeys)),
            key(theme, "⇧P", theme.accent_senary),
            label(theme, "profiles"),
            key(theme, "⇧U", theme.accent_senary),
            label(theme, i18n::t(lang, Key::HbUnset)),
            key(theme, "F", theme.accent_quaternary),
            label(theme, i18n::t(lang, Key::HbFiles)),
            key(theme, "T", theme.accent_quaternary),
            label(theme, i18n::t(lang, Key::HbDevices)),
            key(theme, "⇧B", theme.accent_senary),
            label(theme, i18n::t(lang, Key::HbPublish)),
            key(theme, "V", theme.info),
            label(theme, i18n::t(lang, Key::HbViewLogs)),
            key(theme, "I", theme.info),
            label(theme, i18n::t(lang, Key::HbInfo)),
            key(theme, "⇧E", theme.accent_quaternary),
            label(theme, i18n::t(lang, Key::HbExport)),
            key(theme, "⇧I", theme.accent_quaternary),
            label(theme, i18n::t(lang, Key::HbImport)),
            key(theme, "/", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HbFilter)),
            key(theme, "Supr", theme.error),
            label(theme, i18n::t(lang, Key::HbDelete)),
        ],
        Tab::Snapshots => vec![
            key(theme, "⇧S", theme.accent_quinary),
            label(theme, i18n::t(lang, Key::HbNew)),
            key(theme, "R", theme.warning),
            label(theme, i18n::t(lang, Key::HbRestore)),
            key(theme, "Supr", theme.error),
            label(theme, i18n::t(lang, Key::HbDelete)),
        ],
        Tab::Images => vec![
            key(theme, "N", theme.accent_primary),
            label(theme, i18n::t(lang, Key::HbLaunch)),
            key(theme, "A", theme.accent_quinary),
            label(theme, "alias"),
            key(theme, "W", theme.accent_senary),
            label(theme, i18n::t(lang, Key::HbDownload)),
            key(theme, "I", theme.info),
            label(theme, i18n::t(lang, Key::HbInfo)),
            key(theme, "⇧E", theme.accent_quaternary),
            label(theme, i18n::t(lang, Key::HbExport)),
            key(theme, "⇧I", theme.accent_quaternary),
            label(theme, i18n::t(lang, Key::HbImport)),
            key(theme, "Supr", theme.error),
            label(theme, i18n::t(lang, Key::HbDelete)),
        ],
        Tab::Profiles => vec![
            key(theme, "N", theme.accent_primary),
            label(theme, i18n::t(lang, Key::HbNew)),
            key(theme, "⇧D", theme.accent_senary),
            label(theme, "description"),
            key(theme, "L", theme.accent_senary),
            label(theme, i18n::t(lang, Key::HbLimits)),
            key(theme, "I", theme.info),
            label(theme, i18n::t(lang, Key::HbInfo)),
            key(theme, "Supr", theme.error),
            label(theme, i18n::t(lang, Key::HbDelete)),
        ],
        Tab::Logs => vec![
            key(theme, "R", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HbRefresh)),
            key(theme, "⇧O", theme.info),
            label(theme, i18n::t(lang, Key::HbOperations)),
            key(theme, "⇧W", theme.warning),
            label(theme, i18n::t(lang, Key::HbWarnings)),
        ],
        Tab::Network => vec![
            key(theme, "N", theme.accent_primary),
            label(theme, i18n::t(lang, Key::HbNew)),
            key(theme, "B", theme.accent_quinary),
            label(theme, i18n::t(lang, Key::HbAttach)),
            key(theme, "O", theme.accent_senary),
            label(theme, i18n::t(lang, Key::HbConfigKeys)),
            key(theme, "I", theme.info),
            label(theme, i18n::t(lang, Key::HbInfo)),
            key(theme, "Supr", theme.error),
            label(theme, i18n::t(lang, Key::HbDelete)),
        ],
        Tab::Storage if app.storage_drill.is_some() => vec![
            key(theme, "N", theme.accent_primary),
            label(theme, i18n::t(lang, Key::HbNew)),
            key(theme, "B", theme.accent_quinary),
            label(theme, i18n::t(lang, Key::HbAttach)),
            key(theme, "U", theme.accent_quinary),
            label(theme, i18n::t(lang, Key::HbDetach)),
            key(theme, "M", theme.accent_senary),
            label(theme, i18n::t(lang, Key::HbRename)),
            key(theme, "O", theme.accent_senary),
            label(theme, i18n::t(lang, Key::HbConfigKeys)),
            key(theme, "I", theme.info),
            label(theme, i18n::t(lang, Key::HbInfo)),
            key(theme, "⇧S", theme.accent_quinary),
            label(theme, "snapshot"),
            key(theme, "⇧M", theme.accent_senary),
            label(theme, i18n::t(lang, Key::HbMoveTo)),
            key(theme, "Supr", theme.error),
            label(theme, i18n::t(lang, Key::HbDelete)),
            key(theme, "Esc", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HbBack)),
        ],
        Tab::Storage => vec![
            key(theme, "N", theme.accent_primary),
            label(theme, i18n::t(lang, Key::HbNew)),
            key(theme, "Enter", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HbEnter)),
            key(theme, "I", theme.info),
            label(theme, i18n::t(lang, Key::HbInfo)),
            key(theme, "Supr", theme.error),
            label(theme, i18n::t(lang, Key::HbDelete)),
        ],
        Tab::SystemCheck => vec![
            key(theme, "R", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HbRecheck)),
            key(theme, "Enter", theme.accent_primary),
            label(theme, i18n::t(lang, Key::HbFix)),
        ],
        _ => vec![],
    });

    spans.push(key(theme, "C", theme.accent_primary));
    spans.push(label(theme, i18n::t(lang, Key::HbConfig)));
    spans.push(key(theme, "H", theme.accent_secondary));
    spans.push(label(theme, i18n::t(lang, Key::SidebarHelp)));
    spans.push(key(theme, "Q", theme.error));
    spans.push(label(theme, i18n::t(lang, Key::HbQuit)));

    spans
}

fn key(theme: &Theme, text: &str, color: Color) -> Span<'static> {
    Span::styled(format!(" {text} "), theme.plate_style(color))
}

fn label(theme: &Theme, text: &str) -> Span<'static> {
    Span::styled(format!(" {text}   "), theme.label_style())
}
