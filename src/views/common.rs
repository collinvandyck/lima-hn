use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

use crate::theme::ResolvedTheme;

/// Render an error message in a bordered block.
pub fn render_error(frame: &mut Frame, error: &str, theme: &ResolvedTheme, area: Rect) {
    let widget = Paragraph::new(error).style(theme.error_style()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style())
            .title("Error"),
    );
    frame.render_widget(widget, area);
}
