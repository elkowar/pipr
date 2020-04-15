use super::app::*;
use crate::snippets::Snippet;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    collections::HashMap,
    io::{self, Stdout, Write},
    process::Command,
};
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, ListState, Paragraph, Text};
use tui::{backend::Backend, backend::CrosstermBackend, Frame, Terminal};
use Constraint::*;

fn make_default_block(title: &str, selected: bool) -> Block {
    let title_style = if selected {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::Cyan).bg(Color::Black)
    };

    Block::default().title(title).borders(Borders::ALL).title_style(title_style)
}

pub fn draw_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<(), failure::Error> {
    if let Some(manpage) = &app.opened_manpage {
        execute!(io::stdout(), LeaveAlternateScreen)?;
        Command::new("man").arg(manpage).spawn()?.wait()?;
        app.opened_manpage = None;
        execute!(io::stdout(), EnterAlternateScreen)?;
        terminal.resize(terminal.size()?)?;
    }

    let mut input_field_rect = tui::layout::Rect::new(0, 0, 0, 0);
    terminal.draw(|mut f| {
        let root_rect = f.size();
        let root_rect = Rect::new(1, 1, root_rect.width - 2, root_rect.height - 2);
        match &app.window_state {
            WindowState::Main => {
                let root_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Percentage(if app.snippet_mode { 40 } else { 0 }), Percentage(100)].as_ref())
                    .split(root_rect);

                let exec_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Length(2 + app.input_state.content_lines().len() as u16), Percentage(100)].as_ref())
                    .split(root_chunks[1]);

                if app.snippet_mode {
                    draw_snippet_list(&mut f, root_chunks[0], &app.config.snippets);
                }

                input_field_rect = exec_chunks[0];
                draw_input_field(&mut f, input_field_rect, &app);
                draw_outputs(
                    &mut f,
                    exec_chunks[1],
                    &app.input_state.content_str() == &app.last_executed_cmd,
                    &app.command_output,
                    &app.command_error,
                );
            }
            WindowState::TextView(title, text) => {
                let paragraph_content = [Text::raw(text)];
                f.render_widget(
                    Paragraph::new(paragraph_content.iter()).block(make_default_block(title, true)),
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

    let items = state
        .list
        .iter()
        .map(|entry| str::replace(&entry.as_string(), "\n", " ↵ "))
        .collect::<Vec<String>>();

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

fn draw_snippet_list<B: Backend>(f: &mut Frame<B>, rect: Rect, snippets: &HashMap<char, Snippet>) {
    let snippet_list = snippets
        .iter()
        .map(|(c, snippet)| c.to_string() + ": " + &snippet.text.trim());

    f.render_widget(
        List::new(snippet_list.map(Text::raw)).block(make_default_block("Snippets", false)),
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

    let stdout_item = [Text::raw(stdout)];
    let stdout_title = format!("Output{}", if changed { "" } else { " [+]" });
    f.render_widget(
        Paragraph::new(stdout_item.iter()).block(make_default_block(&stdout_title, false)),
        output_chunks[0],
    );

    if !stderr.is_empty() {
        f.render_widget(
            Paragraph::new([Text::raw(stderr)].iter()).block(make_default_block("Stderr", false)),
            output_chunks[1],
        );
    }
}
