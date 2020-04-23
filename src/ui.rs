use crate::app::command_list_window::CommandListState;
use crate::app::*;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use tui::layout::{Constraint, Corner, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, ListState, Paragraph, Text};
use tui::{backend::Backend, Frame, Terminal};
use Constraint::*;

pub fn draw_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<(), failure::Error> {
    if let Some(mut should_jump_to_other_cmd) = app.should_jump_to_other_cmd.take() {
        execute!(io::stdout(), LeaveAlternateScreen)?;
        should_jump_to_other_cmd.env("MAN_POSIXLY_CORRECT", "1").spawn()?.wait()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        terminal.resize(terminal.size()?)?; // this will redraw the whole screen
    }

    let mut input_field_rect = tui::layout::Rect::new(0, 0, 0, 0);
    terminal.draw(|mut f| {
        let root_rect = f.size();
        let root_rect = Rect::new(1, 1, root_rect.width - 2, root_rect.height - 2);
        match &app.window_state {
            WindowState::Main => {
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

                if let Some(opened_key_select_menu) = &app.opened_key_select_menu {
                    let options = opened_key_select_menu.option_list_strings();
                    f.render_widget(
                        List::new(options.map(Text::raw)).block(make_default_block("Snippets", false)),
                        root_chunks[0],
                    );
                }

                input_field_rect = exec_chunks[0];
                draw_input_field(&mut f, input_field_rect, &app);

                if let Some(autocomplete_state) = &app.autocomplete_state {
                    let mut list_state = ListState::default();
                    list_state.select(Some(autocomplete_state.current_idx));

                    let list_widget = List::new(autocomplete_state.options.iter().map(Text::raw))
                        .highlight_style(Style::default().fg(Color::Black).bg(Color::White))
                        .block(make_default_block("Suggestions", false));
                    f.render_stateful_widget(list_widget, exec_chunks[1], &mut list_state);
                }
                draw_outputs(
                    &mut f,
                    exec_chunks[2],
                    &app.input_state.content_str() == &app.last_executed_cmd,
                    &app.command_output,
                    &app.command_error,
                );
            }
            WindowState::TextView(title, text) => {
                f.render_widget(
                    Paragraph::new([Text::raw(text)].iter()).block(make_default_block(title, true)),
                    root_rect,
                );
            }
            WindowState::BookmarkList(listview_state) => {
                let always_show_preview = app.config.cmdlist_always_show_preview;
                draw_command_list(&mut f, root_rect, always_show_preview, listview_state, "Bookmarks");
            }
            WindowState::HistoryList(listview_state) => {
                let always_show_preview = app.config.cmdlist_always_show_preview;
                draw_command_list(&mut f, root_rect, always_show_preview, listview_state, "History");
            }
        }

        f.render_widget(
            Paragraph::new([Text::raw("Help: F1")].iter()),
            Rect::new(root_rect.width - 10 as u16, root_rect.height as u16, 10, 1),
        );
    })?;

    match app.window_state {
        WindowState::Main => {
            terminal.show_cursor()?;
            let cursor_x = input_field_rect.x + 1 + app.input_state.displayed_cursor_column() as u16;
            let cursor_y = input_field_rect.y + 1 + app.input_state.cursor_line as u16;
            terminal.set_cursor(cursor_x, cursor_y)?;
        }
        _ => terminal.hide_cursor()?,
    }
    Ok(())
}

fn draw_command_list<B: Backend>(f: &mut Frame<B>, rect: Rect, always_show_preview: bool, state: &CommandListState, title: &str) {
    let show_preview = always_show_preview || state.selected_entry().map(|e| e.lines().len() > 1) == Some(true);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Percentage(if show_preview { 60 } else { 100 }), Percentage(100)].as_ref())
        .split(rect);

    let items: Vec<String> = state
        .list
        .iter()
        .map(|entry| entry.as_string().replace("\n", " ↵ "))
        .collect();

    let mut list_state = ListState::default();
    list_state.select(state.selected_idx);

    let list_widget = List::new(items.iter().map(Text::raw))
        .block(make_default_block(title, true))
        .highlight_style(Style::default().modifier(Modifier::ITALIC))
        .highlight_symbol(">>");

    f.render_stateful_widget(list_widget, chunks[0], &mut list_state);

    if show_preview {
        if let Some(selected_content) = state.selected_entry() {
            f.render_widget(
                Paragraph::new([Text::raw(selected_content.as_string())].iter()).block(make_default_block("Preview", false)),
                chunks[1],
            );
        }
    }
}

fn draw_input_field<B: Backend>(f: &mut Frame<B>, rect: Rect, app: &App) {
    let lines = app.input_state.content_lines().iter().map(|line| {
        let mut line = line.clone();
        if line.len() > rect.width as usize - 5 {
            line.truncate(rect.width as usize - 5);
            line.push_str("...");
        }
        line
    });
    let is_bookmarked = app.bookmarks.entries.contains(&app.input_state.content_to_commandentry());
    let input_block_title = format!(
        "Command{}{}{}",
        if is_bookmarked { " [Bookmarked]" } else { "" },
        if app.autoeval_mode { " [Autoeval]" } else { "" },
        if app.autoeval_mode && app.paranoid_history_mode {
            " [Paranoid]"
        } else {
            ""
        }
    );

    f.render_widget(
        List::new(lines.map(Text::raw)).block(make_default_block(&input_block_title, true)),
        rect,
    );
}

fn draw_outputs<B: Backend>(f: &mut Frame<B>, rect: Rect, changed: bool, stdout: &str, stderr: &str) {
    let output_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if stderr.is_empty() {
            [Percentage(100)].as_ref()
        } else {
            [Percentage(50), Percentage(50)].as_ref()
        })
        .split(rect);

    let stdout_title = format!("Output{}", if changed { "" } else { " [+]" });
    f.render_widget(
        Paragraph::new([Text::raw(stdout)].iter()).block(make_default_block(&stdout_title, false)),
        output_chunks[0],
    );

    if !stderr.is_empty() {
        f.render_widget(
            Paragraph::new([Text::raw(stderr)].iter()).block(make_default_block("Stderr", false)),
            output_chunks[1],
        );
    }
}

fn make_default_block(title: &str, selected: bool) -> Block {
    let title_style = if selected {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::Cyan).bg(Color::Black)
    };

    Block::default().title(title).borders(Borders::ALL).title_style(title_style)
}
