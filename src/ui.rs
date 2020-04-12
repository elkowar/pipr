use super::app::*;
use std::io::{self, Stdout, Write};
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, Paragraph, SelectableList, Text, Widget};
use tui::{backend::Backend, backend::CrosstermBackend, Frame, Terminal};
use Constraint::*;

const HELP_TEXT: &str = "\
F1         Show/hide help
F2         Toggle autoeval
Ctrl+B     Show/hide bookmarks
Ctrl+S     Save bookmark
Alt+Return Newline
Ctrl+X     Clear Command
Ctrl+P     Previous in history
Ctrl+N     Next in history

Config file is in
~/.config/pipr/pipr.toml";

fn make_default_block(title: &str, selected: bool) -> Block {
    let title_style = if selected {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::Cyan).bg(Color::Black)
    };

    Block::default().title(title).borders(Borders::ALL).title_style(title_style)
}

pub fn draw_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<(), failure::Error> {
    let mut input_field_rect = tui::layout::Rect::new(0, 0, 0, 0);
    terminal.draw(|mut f| match &app.window_state {
        WindowState::Main => {
            let exec_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Length(2 + app.input_state.content_lines().len() as u16), Percentage(100)].as_ref())
                .split(f.size());

            input_field_rect = exec_chunks[0];
            draw_input_field(&mut f, input_field_rect, &app);
            draw_outputs(&mut f, exec_chunks[1], &app.command_output, &app.command_error);
        }
        WindowState::TextView(text) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Percentage(100)].as_ref())
                .split(f.size());

            Paragraph::new([Text::raw(text)].iter())
                .block(make_default_block("TODO SET THIS", true))
                .render(&mut f, chunks[0]);
        }
        WindowState::BookmarkList(listview_state) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Percentage(100)].as_ref())
                .split(f.size());

            draw_command_list(&mut f, chunks[0], listview_state, "Bookmarks");
        }
        WindowState::HistoryList(listview_state) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Percentage(100)].as_ref())
                .split(f.size());

            draw_command_list(&mut f, chunks[0], listview_state, "History");
        }
        _ => {}
    })?;

    // move cursor to where it belongs.
    terminal.backend_mut().write(
        format!(
            "{}",
            crossterm::cursor::MoveTo(
                input_field_rect.x + 1 + app.input_state.displayed_cursor_column() as u16,
                input_field_rect.y + 1 + app.input_state.cursor_line as u16,
            )
        )
        .as_bytes(),
    )?;
    // immediately _show_ the moved cursor where it now should be
    io::stdout().flush().ok();
    Ok(())
}

fn draw_command_list<B: Backend>(mut f: &mut Frame<B>, rect: Rect, state: &CommandListState, title: &str) {
    SelectableList::default()
        .block(make_default_block(title, true))
        .items(state.list.as_strings().as_slice())
        .select(state.selected_idx)
        .highlight_style(Style::default().modifier(Modifier::ITALIC))
        .highlight_symbol(">>")
        .render(&mut f, rect);
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
    let output_constraints = if stderr.is_empty() {
        [Percentage(100)].as_ref()
    } else {
        [Percentage(50), Percentage(50)].as_ref()
    };

    let output_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(output_constraints)
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

fn draw_shortcuts<B: Backend>(mut f: &mut Frame<B>, rect: Rect) {
    Paragraph::new([Text::raw(HELP_TEXT)].iter())
        .block(make_default_block("Help", false))
        .render(&mut f, rect);
}
