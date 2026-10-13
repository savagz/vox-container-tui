use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Row, Table},
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::ui::format;
use crate::ui::widgets::panel;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let accent = theme.tab_accent(5);

    let block = panel::block(theme, super::Tab::Images.title(app.lang), accent, false).title_bottom(panel::readout(
        theme,
        &i18n::t(lang, Key::ImagesDetailTemplate).replacen("{}", &app.inventory.images.len().to_string(), 1),
        theme.fg_muted,
    ));

    let header = Row::new([
        "ALIAS",
        i18n::t(lang, Key::ImagesColDescription),
        "FINGERPRINT",
        "ARCH",
        i18n::t(lang, Key::ImagesColSize),
        i18n::t(lang, Key::ImagesColUploaded),
    ])
    .style(theme.table_header_style());

    let selected = app.image_row;
    let rows = app.inventory.images.iter().enumerate().map(|(i, img)| {
        let mut alias = img
            .aliases
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| i18n::t(lang, Key::ImagesNoAlias).to_string());
        if img.public {
            alias.push_str(i18n::t(lang, Key::ImagesPublicSuffix));
        }
        let description = img.description().unwrap_or_else(|| i18n::t(lang, Key::ImagesNoDescription));
        let short_fp: String = img.fingerprint.chars().take(12).collect();

        let row = Row::new([
            Span::styled(alias, theme.style_for_bold()),
            Span::styled(description.to_string(), theme.value_style()),
            Span::styled(short_fp, theme.readout_style()),
            Span::styled(img.architecture.clone(), theme.label_style()),
            Span::styled(format::bytes(img.size), theme.readout_style()),
            Span::styled(img.uploaded_at.clone(), theme.label_style()),
        ]);
        if i == selected {
            row.style(theme.selected_row_style())
        } else {
            row
        }
    });

    let widths = [
        Constraint::Percentage(16),
        Constraint::Percentage(28),
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Percentage(18),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1).block(block);
    frame.render_widget(table, area);
}
