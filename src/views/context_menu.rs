use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::app::App;
use crate::help::context_menu_help;
use crate::keys::context_menu_keymap;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let Some(menu) = &app.context_menu else {
        return;
    };

    let theme = &app.theme;

    // Calculate centered popup size
    let popup_width = 35.min(area.width.saturating_sub(4));
    #[allow(clippy::cast_possible_truncation)]
    let popup_height = ((menu.items.len() + 3) as u16).min(area.height.saturating_sub(4));
    let popup_area = centered_rect(popup_width, popup_height, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    // Split popup into list and help areas
    let chunks = Layout::vertical([
        Constraint::Min(0),    // Menu items
        Constraint::Length(1), // Help line
    ])
    .split(popup_area);

    // Build menu items
    let items: Vec<ListItem> = menu
        .items
        .iter()
        .map(|item| ListItem::new(item.label()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style())
                .title("Actions"),
        )
        .highlight_style(theme.selection_style())
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(menu.selected));
    frame.render_stateful_widget(list, chunks[0], &mut state);

    // Help line
    let keymap = context_menu_keymap();
    let help_text = context_menu_help().format(&keymap, true);
    let help = Paragraph::new(help_text).style(theme.dim_style());
    frame.render_widget(help, chunks[1]);
}

const fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{StoryBuilder, TestAppBuilder};
    use crate::views::tests::render_to_string;

    #[test]
    fn test_context_menu_renders() {
        let stories = vec![
            StoryBuilder::new()
                .id(1)
                .title("Test Story")
                .url("https://example.com")
                .author("dang")
                .build(),
        ];
        let mut app = TestAppBuilder::new().with_stories(stories).build();
        app.update(crate::app::Message::OpenContextMenu);

        let output = render_to_string(80, 24, |frame| {
            render(frame, &app, frame.area());
        });

        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_context_menu_selection() {
        let stories = vec![
            StoryBuilder::new()
                .id(1)
                .title("Test Story")
                .url("https://example.com")
                .author("pg")
                .build(),
        ];
        let mut app = TestAppBuilder::new().with_stories(stories).build();
        app.update(crate::app::Message::OpenContextMenu);
        app.update(crate::app::Message::ContextMenuDown);

        let output = render_to_string(80, 24, |frame| {
            render(frame, &app, frame.area());
        });

        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_context_menu_self_post_hides_domain_filter() {
        let stories = vec![
            StoryBuilder::new()
                .id(1)
                .title("Ask HN: Self post")
                .author("someone")
                .no_url() // Self post has no URL
                .build(),
        ];
        let mut app = TestAppBuilder::new().with_stories(stories).build();
        app.update(crate::app::Message::OpenContextMenu);

        let output = render_to_string(80, 24, |frame| {
            render(frame, &app, frame.area());
        });

        // Should only have 2 items (no "filter by domain")
        assert_eq!(app.context_menu.as_ref().unwrap().items.len(), 2);
        insta::assert_snapshot!(output);
    }
}
