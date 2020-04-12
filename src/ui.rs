use super::app::*;
use std::io::{self, Stdout, Write};
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, Paragraph, SelectableList, Text, Widget};
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

pub fn draw_app(mut terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<(), failure::Error> {
    let mut input_field_rect = tui::layout::Rect::new(0, 0, 0, 0);
    terminal.draw(|mut f| {
        let root_rect = f.size();
        let root_rect = Rect::new(1, 1, root_rect.width - 2, root_rect.height - 2);
        match &app.window_state {
            WindowState::Main => {
                let exec_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Length(2 + app.input_state.content_lines().len() as u16), Percentage(100)].as_ref())
                    .split(root_rect);

                input_field_rect = exec_chunks[0];
                draw_input_field(&mut f, input_field_rect, &app);
                draw_outputs(&mut f, exec_chunks[1], &app.command_output, &app.command_error);
            }
            WindowState::TextView(title, text) => {
                Paragraph::new([Text::raw(text)].iter())
                    .block(make_default_block(title, true))
                    .render(&mut f, root_rect);
            }
            WindowState::BookmarkList(listview_state) => {
                draw_command_list(&mut f, root_rect, listview_state, "Bookmarks");
            }
            WindowState::HistoryList(listview_state) => {
                draw_command_list(&mut f, root_rect, listview_state, "History");
            }
        }

        Paragraph::new([Text::raw("Help: F1")].iter())
            .render(&mut f, Rect::new(root_rect.width - 10 as u16, root_rect.height as u16, 10, 1))
    })?;

    match app.window_state {
        WindowState::Main => {
            set_crossterm_cursor_visibility(&mut terminal, true);
            let cursor_x = input_field_rect.x + 1 + app.input_state.displayed_cursor_column() as u16;
            let cursor_y = input_field_rect.y + 1 + app.input_state.cursor_line as u16;
            set_crossterm_cursor_position(&mut terminal, cursor_x, cursor_y);
        }
        _ => set_crossterm_cursor_visibility(&mut terminal, false),
    }
    Ok(())
}

fn draw_command_list<B: Backend>(mut f: &mut Frame<B>, rect: Rect, state: &CommandListState, title: &str) {
    let needs_preview = state.selected_entry().map(|e| e.lines().len() > 1) == Some(true);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Percentage(if needs_preview { 60 } else { 100 }), Percentage(100)].as_ref())
        .split(rect);

    let items = state
        .list
        .iter()
        .map(|entry| str::replace(&entry.as_string(), "\n", " ↵ "))
        .collect::<Vec<String>>();
    SelectableList::default()
        .block(make_default_block(title, true))
        .items(items.as_slice())
        .select(state.selected_idx)
        .highlight_style(Style::default().modifier(Modifier::ITALIC))
        .highlight_symbol(">>")
        .render(&mut f, chunks[0]);

    if needs_preview {
        if let Some(selected_content) = state.selected_entry() {
            Paragraph::new([Text::raw(selected_content.as_string())].iter())
                .block(make_default_block("Preview", false))
                .render(&mut f, chunks[1]);
        }
    }
}

fn draw_input_field<B: Backend>(mut f: &mut Frame<B>, rect: Rect, app: &App) {
    let lines = app.input_state.content_lines().into_iter().map(|mut line| {
        if line.len() > rect.width as usize - 5 {
            line.truncate(rect.width as usize - 5);
            line.push_str("...");
        }
        line
    });

    List::new(lines.map(Text::raw))
        .block(make_default_block(
            &format!("Command{}", if app.autoeval_mode { " [Autoeval]" } else { "" }),
            true,
        ))
        .render(&mut f, rect);
}

fn draw_outputs<B: Backend>(mut f: &mut Frame<B>, rect: Rect, stdout: &str, stderr: &str) {
    let output_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if stderr.is_empty() {
            [Percentage(100)].as_ref()
        } else {
            [Percentage(50), Percentage(50)].as_ref()
        })
        .split(rect);

    Paragraph::new([Text::raw(stdout)].iter())
        .block(make_default_block("Output", false))
        .render(&mut f, output_chunks[0]);

    if !stderr.is_empty() {
        Paragraph::new([Text::raw(stderr)].iter())
            .block(make_default_block("Stderr", false))
            .render(&mut f, output_chunks[1]);
    }
}

fn set_crossterm_cursor_position(terminal: &mut Terminal<CrosstermBackend<Stdout>>, x: u16, y: u16) {
    terminal
        .backend_mut()
        .write(format!("{}", crossterm::cursor::MoveTo(x, y)).as_bytes())
        .unwrap();
    io::stdout().flush().ok();
}

fn set_crossterm_cursor_visibility(terminal: &mut Terminal<CrosstermBackend<Stdout>>, visible: bool) {
    let command = match visible {
        true => format!("{}", crossterm::cursor::Show),
        false => format!("{}", crossterm::cursor::Hide),
    };
    terminal.backend_mut().write(command.as_bytes()).unwrap();
    io::stdout().flush().ok();
}
