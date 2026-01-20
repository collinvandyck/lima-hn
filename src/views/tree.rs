//! Tree rendering utilities for comment threads.
//!
//! Builds ASCII tree prefixes (│, ├─, └─) for nested comment display.

use ratatui::{style::Style, text::Span};

use crate::api::Comment;

/// Compute tree context for visible comments.
///
/// For each visible comment, returns a vector of booleans indicating whether
/// there are more siblings at each depth level. This is used to determine
/// whether to draw │ (continuation) or leave blank at each indentation level.
pub fn compute_tree_context(comments: &[Comment], visible_indices: &[usize]) -> Vec<Vec<bool>> {
    visible_indices
        .iter()
        .enumerate()
        .map(|(vis_idx, &actual_idx)| {
            let depth = comments[actual_idx].depth;

            (0..=depth)
                .map(|check_depth| {
                    for &future_idx in &visible_indices[vis_idx + 1..] {
                        let future_depth = comments[future_idx].depth;
                        if future_depth == check_depth {
                            return true;
                        }
                        if future_depth < check_depth {
                            return false;
                        }
                    }
                    false
                })
                .collect()
        })
        .collect()
}

/// Build the tree prefix for a comment's meta line (author, time).
///
/// Returns styled spans with the appropriate tree characters:
/// - `├─` if there are more siblings at this depth
/// - `└─` if this is the last sibling at this depth
/// - `│` for ancestor continuation
///
/// Each segment is colored according to its depth level.
pub fn build_meta_tree_prefix<F>(
    depth: usize,
    has_more_at_depth: &[bool],
    depth_color: F,
) -> Vec<Span<'static>>
where
    F: Fn(usize) -> ratatui::style::Color,
{
    if depth == 0 {
        return vec![];
    }
    let mut spans = Vec::with_capacity(depth);
    // Add ancestor continuation (│ or spaces) for depths 1 to depth-1
    for d in 1..depth {
        let text = if has_more_at_depth.get(d).copied().unwrap_or(false) {
            " │  "
        } else {
            "    "
        };
        spans.push(Span::styled(text, Style::default().fg(depth_color(d))));
    }
    // Add connector for current depth
    let connector = if has_more_at_depth.get(depth).copied().unwrap_or(false) {
        " ├─ "
    } else {
        " └─ "
    };
    spans.push(Span::styled(
        connector,
        Style::default().fg(depth_color(depth)),
    ));
    spans
}

/// Build the tree prefix for comment text lines.
///
/// Similar to meta prefix but extends one level deeper to show
/// continuation for the comment's own children if expanded.
///
/// Each segment is colored according to its depth level.
pub fn build_text_prefix<F>(
    depth: usize,
    has_more_at_depth: &[bool],
    has_children: bool,
    depth_color: F,
) -> Vec<Span<'static>>
where
    F: Fn(usize) -> ratatui::style::Color,
{
    let mut spans = Vec::with_capacity(depth + 2);
    // Add ancestor continuation for depths 1 to depth
    for d in 1..=depth {
        let text = if has_more_at_depth.get(d).copied().unwrap_or(false) {
            " │  "
        } else {
            "    "
        };
        spans.push(Span::styled(text, Style::default().fg(depth_color(d))));
    }
    // Add own tree line if has visible children (colored as depth + 1)
    let child_text = if has_children { " │  " } else { "    " };
    spans.push(Span::styled(
        child_text,
        Style::default().fg(depth_color(depth + 1)),
    ));
    spans
}

/// Build the tree prefix for the empty line after a comment.
///
/// Shows tree continuation lines but no connector.
///
/// Each segment is colored according to its depth level.
pub fn build_empty_line_prefix<F>(
    depth: usize,
    has_more_at_depth: &[bool],
    has_children: bool,
    depth_color: F,
) -> Vec<Span<'static>>
where
    F: Fn(usize) -> ratatui::style::Color,
{
    let mut spans = Vec::with_capacity(depth + 2);
    // Add continuation markers for depths 1 to depth
    for d in 1..=depth {
        let text = if has_more_at_depth.get(d).copied().unwrap_or(false) {
            " │  "
        } else {
            "    "
        };
        spans.push(Span::styled(text, Style::default().fg(depth_color(d))));
    }
    // Add own tree line if has visible children (colored as depth + 1)
    if has_children {
        spans.push(Span::styled(
            " │",
            Style::default().fg(depth_color(depth + 1)),
        ));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::CommentBuilder;

    #[test]
    fn test_compute_tree_context_single_comment() {
        let comments = vec![CommentBuilder::new().id(1).depth(0).build()];
        let visible = vec![0];
        let context = compute_tree_context(&comments, &visible);

        assert_eq!(context.len(), 1);
        assert_eq!(context[0], vec![false]); // No more at depth 0
    }

    #[test]
    fn test_compute_tree_context_siblings() {
        let comments = vec![
            CommentBuilder::new().id(1).depth(0).build(),
            CommentBuilder::new().id(2).depth(0).build(),
        ];
        let visible = vec![0, 1];
        let context = compute_tree_context(&comments, &visible);

        assert_eq!(context[0], vec![true]); // More siblings at depth 0
        assert_eq!(context[1], vec![false]); // Last at depth 0
    }

    #[test]
    fn test_compute_tree_context_nested() {
        let comments = vec![
            CommentBuilder::new().id(1).depth(0).kids(vec![2]).build(),
            CommentBuilder::new().id(2).depth(1).build(),
            CommentBuilder::new().id(3).depth(0).build(),
        ];
        let visible = vec![0, 1, 2];
        let context = compute_tree_context(&comments, &visible);

        assert_eq!(context[0], vec![true]); // More at depth 0
        assert_eq!(context[1], vec![true, false]); // More at 0, none at 1
        assert_eq!(context[2], vec![false]); // Last at depth 0
    }

    fn white(_: usize) -> ratatui::style::Color {
        ratatui::style::Color::White
    }

    fn spans_to_string(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_build_meta_tree_prefix_root() {
        let spans = build_meta_tree_prefix(0, &[false], white);
        assert!(spans.is_empty());
    }

    #[test]
    fn test_build_meta_tree_prefix_with_sibling() {
        let spans = build_meta_tree_prefix(1, &[false, true], white);
        assert_eq!(spans_to_string(&spans), " ├─ ");
    }

    #[test]
    fn test_build_meta_tree_prefix_last_sibling() {
        let spans = build_meta_tree_prefix(1, &[false, false], white);
        assert_eq!(spans_to_string(&spans), " └─ ");
    }

    #[test]
    fn test_build_text_prefix_with_children() {
        let spans = build_text_prefix(0, &[false], true, white);
        assert_eq!(spans_to_string(&spans), " │  ");
    }

    #[test]
    fn test_build_text_prefix_no_children() {
        let spans = build_text_prefix(0, &[false], false, white);
        assert_eq!(spans_to_string(&spans), "    ");
    }
}
