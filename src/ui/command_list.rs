use crate::app::command_list_window::CommandListState;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Span,
    widgets::{List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::ui::make_default_block;

/// Draw the command list UI (used for both bookmarks and history)
pub fn draw_command_list(f: &mut Frame, rect: Rect, always_show_preview: bool, state: &CommandListState, title: &str) {
    let show_preview = always_show_preview || state.selected_entry().map(|e| e.lines().len() > 1) == Some(true);

    let [list_chunk, preview_chunk] = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(if show_preview { 60 } else { 100 }),
                Constraint::Percentage(100),
            ]
            .as_ref(),
        )
        .areas(rect);

    let items = state
        .list
        .iter()
        .map(|entry| entry.as_string().replace("\n", " ↵ "))
        .map(|entry| ListItem::new(Span::raw(entry)))
        .collect::<Vec<_>>();

    let mut list_state = ListState::default();
    list_state.select(state.selected_idx);

    use ratatui::style::{Modifier, Style};

    let list_widget = List::new(items)
        .block(make_default_block(title, true))
        .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
        .highlight_symbol(">>");

    f.render_stateful_widget(list_widget, list_chunk, &mut list_state);

    if show_preview {
        if let Some(selected_content) = state.selected_entry() {
            f.render_widget(
                Paragraph::new(selected_content.as_string().as_str()).block(make_default_block("Preview", false)),
                preview_chunk,
            );
        }
    }
}
