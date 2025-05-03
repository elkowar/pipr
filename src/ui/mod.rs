use crate::app::{App, WindowState};

use command_list::draw_command_list;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use input_field::draw_input_field;
use outputs::draw_outputs;
use ratatui::{
    backend::Backend,
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders},
    Terminal,
};
use std::io::{self, Write};
use syntect::{
    highlighting::{self, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

pub mod command_list;
pub mod input_field;
pub mod outputs;

lazy_static::lazy_static! {
    pub static ref THEME_SET: ThemeSet = ThemeSet::load_defaults();
    pub static ref SYNTAX_SET: SyntaxSet = SyntaxSet::load_defaults_newlines();
    pub static ref THEME: &'static syntect::highlighting::Theme = THEME_SET.themes.get("base16-ocean.dark").unwrap();
    pub static ref SH_SYNTAX: &'static SyntaxReference = SYNTAX_SET.find_syntax_by_extension("sh").unwrap();
    pub static ref PLAINTEXT_SYNTAX: &'static SyntaxReference = SYNTAX_SET.find_syntax_plain_text();
}

/// Draw the application UI
///
/// This is the main entry point for rendering the UI.
/// It handles different window states and manages the terminal.
pub fn draw_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> anyhow::Result<()> {
    // Handle command execution that jumps to other programs (like man pages)
    if let Some((stdin_content, mut should_jump_to_other_cmd)) = app.should_jump_to_other_cmd.take() {
        execute!(io::stdout(), LeaveAlternateScreen)?;
        let mut child = should_jump_to_other_cmd.env("MAN_POSIXLY_CORRECT", "1").spawn()?;
        if let Some(stdin_content) = stdin_content {
            let _ = child
                .stdin
                .take()
                .expect("Command given to should_jump_to_other_cmd did not provide stdin pipe")
                .write_all(stdin_content.as_bytes());
        }
        child.wait()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        let size = terminal.size()?;
        let rect = ratatui::layout::Rect::new(0, 0, size.width, size.height);
        terminal.resize(rect)?; // this will redraw the whole screen
    }

    let mut input_field_rect = ratatui::layout::Rect::new(0, 0, 0, 0);
    terminal.draw(|f| {
        let root_rect = f.area();
        let root_rect = ratatui::layout::Rect::new(1, 1, root_rect.width - 2, root_rect.height - 2);

        match &app.window_state {
            WindowState::Main => {
                use ratatui::layout::{Constraint::*, Direction, Layout};

                // Split screen for key select menu if needed
                let root_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(
                        [
                            Percentage(if app.opened_key_select_menu.is_some() { 40 } else { 0 }),
                            Percentage(100),
                        ]
                        .as_ref(),
                    )
                    .split(root_rect);

                // Layout for main content area
                let exec_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(
                        [
                            Length(2 + app.input_state.content_lines().len() as u16),
                            Length(if let Some(state) = &app.autocomplete_state {
                                (state.options.len().min(5) + 2) as u16
                            } else {
                                0
                            }),
                            Percentage(100),
                        ]
                        .as_ref(),
                    )
                    .split(root_chunks[1]);

                // Render key select menu if open
                if let Some(opened_key_select_menu) = &app.opened_key_select_menu {
                    use ratatui::text::Span;
                    use ratatui::widgets::{List, ListItem};

                    let options = opened_key_select_menu
                        .option_list_strings()
                        .map(|opt| ListItem::new(Span::raw(opt)))
                        .collect::<Vec<_>>();

                    f.render_widget(List::new(options).block(make_default_block("Open in", false)), root_chunks[0]);
                }

                // Save input field rect for cursor positioning
                input_field_rect = exec_chunks[0];

                // Draw the main components
                draw_input_field(f, input_field_rect, app);

                // Draw autocomplete suggestions if available
                if let Some(autocomplete_state) = &app.autocomplete_state {
                    use ratatui::style::{Color, Style};
                    use ratatui::widgets::{List, ListItem, ListState};

                    let mut list_state = ListState::default();
                    list_state.select(Some(autocomplete_state.current_idx));

                    let list_widget = List::new(
                        autocomplete_state
                            .options
                            .iter()
                            .map(|x| ListItem::new(x.as_str()))
                            .collect::<Vec<_>>(),
                    )
                    .highlight_style(Style::default().fg(Color::Black).bg(Color::White))
                    .block(make_default_block("Suggestions", false));
                    f.render_stateful_widget(list_widget, exec_chunks[1], &mut list_state);
                }

                // Draw command outputs
                draw_outputs(
                    f,
                    exec_chunks[2],
                    app.input_state.content_str() == app.last_executed_cmd,
                    app.is_processing_state,
                    &app.command_output,
                    &app.command_error,
                );

                // Position cursor at current editing position
                let cursor_x = input_field_rect.x + 1 + app.input_state.displayed_cursor_column() as u16;
                let cursor_y = input_field_rect.y + 1 + app.input_state.cursor_line as u16;
                f.set_cursor_position((cursor_x, cursor_y));
            }
            WindowState::TextView(title, text) => {
                use ratatui::widgets::Paragraph;

                f.render_widget(
                    Paragraph::new(text.as_str()).block(make_default_block(title, true)),
                    root_rect,
                );
            }
            WindowState::BookmarkList(listview_state) => {
                let always_show_preview = app.config.cmdlist_always_show_preview;
                draw_command_list(f, root_rect, always_show_preview, listview_state, "Bookmarks");
            }
            WindowState::HistoryList(listview_state) => {
                let always_show_preview = app.config.cmdlist_always_show_preview;
                draw_command_list(f, root_rect, always_show_preview, listview_state, "History");
            }
        }

        // Help message always stays in the bottom right
        use ratatui::widgets::Paragraph;

        f.render_widget(
            Paragraph::new("Help: F1"),
            ratatui::layout::Rect::new(root_rect.width - 10_u16, root_rect.height, 10, 1),
        );
    })?;

    Ok(())
}

/// Converts syntect highlighting style to ratatui style
pub fn highlight_style_to_ratatui_style(style: &highlighting::Style) -> Style {
    let fg = style.foreground;
    Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b)).bg(Color::Reset)
}

/// Creates a default styled block with a title
pub fn make_default_block(title: &str, selected: bool) -> Block {
    let title_style = if selected {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::Cyan).bg(Color::Reset)
    };

    Block::default()
        .title(Span::styled(format!(" {} ", title), title_style))
        .borders(Borders::ALL)
}

/// Display an animation indicator, state being the current frame of the 6-frame animation.
pub fn display_processing_state(state: Option<u8>) -> &'static str {
    match state {
        Some(0) => " ⠟",
        Some(1) => " ⠯",
        Some(2) => " ⠷",
        Some(3) => " ⠾",
        Some(4) => " ⠽",
        Some(5) => " ⠻",
        _ => "",
    }
}

/// Truncates a string to a specific length and adds an ellipsis if needed
pub fn truncate_with_ellipsis(mut line: String, length: usize) -> String {
    if line.len() > length - 5 {
        line.truncate(length - 5);
        line.push_str("...");
    }
    line
}
