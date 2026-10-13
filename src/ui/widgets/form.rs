use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::app::{ActionForm, FormKind};
use crate::i18n::{self, Key, Lang};
use crate::theme::Theme;

use super::panel;

fn title_for(kind: FormKind, lang: Lang) -> &'static str {
    match kind {
        FormKind::NewInstance => i18n::t(lang, Key::FormTitleNewInstance),
        FormKind::Clone => i18n::t(lang, Key::FormTitleClone),
        FormKind::Rename => i18n::t(lang, Key::FormTitleRename),
        FormKind::Snapshot => i18n::t(lang, Key::FormTitleSnapshot),
        FormKind::Limits => i18n::t(lang, Key::FormTitleLimits),
        FormKind::ConfigKeys => i18n::t(lang, Key::FormTitleConfigKeys),
        FormKind::Alias => i18n::t(lang, Key::FormTitleAlias),
        FormKind::NewProfile => i18n::t(lang, Key::FormTitleNewProfile),
        FormKind::AssignProfiles => i18n::t(lang, Key::FormTitleAssignProfiles),
        FormKind::EditProfileDescription => i18n::t(lang, Key::FormTitleEditProfileDescription),
        FormKind::ProfileLimits => i18n::t(lang, Key::FormTitleProfileLimits),
        FormKind::NewVolume => i18n::t(lang, Key::FormTitleNewVolume),
        FormKind::AttachVolume => i18n::t(lang, Key::FormTitleAttachVolume),
        FormKind::DetachVolume => i18n::t(lang, Key::FormTitleDetachVolume),
        FormKind::NewPool => i18n::t(lang, Key::FormTitleNewPool),
        FormKind::VolumeConfigKeys => i18n::t(lang, Key::FormTitleVolumeConfigKeys),
        FormKind::RenameVolume => i18n::t(lang, Key::FormTitleRenameVolume),
        FormKind::ExportInstance => i18n::t(lang, Key::FormTitleExportInstance),
        FormKind::ImportBackup => i18n::t(lang, Key::FormTitleImportBackup),
        FormKind::NewNetwork => i18n::t(lang, Key::FormTitleNewNetwork),
        FormKind::AttachNetwork => i18n::t(lang, Key::FormTitleAttachNetwork),
        FormKind::UnsetConfigKeys => i18n::t(lang, Key::FormTitleUnsetConfigKeys),
        FormKind::TransferFile => i18n::t(lang, Key::FormTitleTransferFile),
        FormKind::Device => i18n::t(lang, Key::FormTitleDevice),
        FormKind::NetworkConfigKeys => i18n::t(lang, Key::FormTitleNetworkConfigKeys),
        FormKind::PublishInstance => i18n::t(lang, Key::FormTitlePublishInstance),
        FormKind::ExportImage => i18n::t(lang, Key::FormTitleExportImage),
        FormKind::ImportImage => i18n::t(lang, Key::FormTitleImportImage),
        FormKind::SnapshotVolume => i18n::t(lang, Key::FormTitleSnapshotVolume),
        FormKind::MoveVolume => i18n::t(lang, Key::FormTitleMoveVolume),
    }
}

/// Modal genérico de 1-2 campos, reusado por crear/clonar/snapshotear/limitar.
pub fn render(frame: &mut Frame, area: Rect, form: &ActionForm, theme: &Theme, lang: Lang) {
    // Los formularios largos (nueva instancia con todas sus opciones) no
    // entran en el 40% de alto que alcanza para 1-3 campos.
    let height_pct = if form.labels.len() > 3 { 70 } else { 40 };
    let modal = panel::centered(area, 70, height_pct);
    frame.render_widget(Clear, modal);

    let accent = theme.accent_primary;
    let mut block = panel::block(theme, title_for(form.kind, lang), accent, true);
    if !form.target.is_empty() {
        block = block.title_bottom(panel::readout(theme, &form.target, theme.fg_muted));
    }
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let mut constraints: Vec<Constraint> = form.labels.iter().map(|_| Constraint::Length(1)).collect();
    constraints.push(Constraint::Length(1));
    constraints.push(Constraint::Min(1));
    constraints.push(Constraint::Length(1));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(constraints)
        .split(inner);

    for (i, label) in form.labels.iter().enumerate() {
        let focused = form.focus == i;
        let marker = if focused { "▌" } else { " " };
        let value_style = if focused {
            theme.selected_row_style()
        } else {
            theme.value_style()
        };
        let line = Line::from(vec![
            Span::styled(marker, theme.style_for_color(accent)),
            Span::styled(format!("{label:<24}"), theme.label_style()),
            Span::styled("│ ", theme.style_for_color(theme.border)),
            Span::styled(form.values[i].clone(), value_style),
            if focused {
                let marker = if form.select_options[i].is_some() { " ◀ ▶" } else { "▏" };
                Span::styled(marker, theme.style_for_color(accent))
            } else {
                Span::raw("")
            },
        ]);
        frame.render_widget(Paragraph::new(line), rows[i]);
    }

    if let Some(error) = &form.error {
        frame.render_widget(
            Paragraph::new(Span::styled(error.clone(), theme.style_for(crate::theme::Status::Error))),
            rows[form.labels.len() + 1],
        );
    }

    let keys = Line::from(vec![
        Span::styled(" TAB ", theme.plate_style(theme.accent_secondary)),
        Span::styled(i18n::t(lang, Key::FormNextField), theme.label_style()),
        Span::styled(" ENTER ", theme.plate_style(theme.ok)),
        Span::styled(i18n::t(lang, Key::Confirm), theme.label_style()),
        Span::styled(" ESC ", theme.plate_style(theme.accent_tertiary)),
        Span::styled(i18n::t(lang, Key::Cancel), theme.label_style()),
    ]);
    frame.render_widget(Paragraph::new(keys), rows[form.labels.len() + 2]);
}
