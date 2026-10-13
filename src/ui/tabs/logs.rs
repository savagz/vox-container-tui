use ratatui::{
    Frame,
    layout::Rect,
    widgets::Paragraph,
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::ui::widgets::panel;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let accent = theme.tab_accent(6);

    let instance_name = app.selected_instance_name.clone().unwrap_or_else(|| "—".to_string());
    let block = panel::block(theme, "logs", accent, false).title_bottom(panel::readout(
        theme,
        &i18n::t(lang, Key::LogsInstanceRefreshTemplate).replacen("{}", &instance_name, 1),
        theme.fg_muted,
    ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = if app.log_loading {
        i18n::t(lang, Key::LogsLoading).to_string()
    } else {
        match &app.log_text {
            Some(log) => log.clone(),
            None => i18n::t(lang, Key::LogsNoneLoaded).to_string(),
        }
    };

    frame.render_widget(
        Paragraph::new(text).style(theme.value_style()).scroll((0, 0)),
        inner,
    );
}
