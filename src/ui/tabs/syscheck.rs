use ratatui::{Frame, layout::Rect};

use crate::app::App;
use crate::ui::setup;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    setup::render_panel(frame, area, &app.system_check, &app.theme);
}
