use super::app::*;
use std::io::{self, Stdout, Write};
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, Paragraph, SelectableList, Text, Widget};
use tui::{backend::Backend, backend::CrosstermBackend, Frame, Terminal};
use Constraint::*;

fn make_default_block(title: &str, selected: bool) -> Block {
    let title_style = if selected {
        Style::default().fg(Color::Black).bg(Color::White)
    } else {
        Style::default().fg(Color::White).bg(Color::Black)
    };

    Block::default().title(title).borders(Borders::ALL).title_style(title_style)
}

pub fn draw_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<(), failure::Error> {
    let mut input_field_rect = tui::layout::Rect::new(0, 0, 0, 0);
    terminal.draw(|mut f| {
        let root_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Percentage(20), Percentage(80)].as_ref())
            .margin(1)
            .split(f.size());

        let exec_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Length(2 + app.input_state.content_lines().len() as u16),
                    Length(3),
                    Percentage(100),
                ]
                .as_ref(),
            )
            .split(root_chunks[1]);

        input_field_rect = exec_chunks[0];

        draw_bookmark_list(&mut f, root_chunks[0], &app);
        draw_input_field(&mut f, input_field_rect, &app);
        draw_outputs(&mut f, exec_chunks[2], &app.command_output, &app.command_error);
        draw_config(&mut f, exec_chunks[1], app.selected_area, app.autoeval_mode);
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

fn draw_bookmark_list<B: Backend>(mut f: &mut Frame<B>, rect: Rect, app: &App) {
    SelectableList::default()
        .block(make_default_block("Bookmarks", app.selected_area == UIArea::BookmarkList))
        .items(app.bookmarks.as_strings().as_slice())
        .select(if app.selected_area == UIArea::BookmarkList {
            app.selected_bookmark_idx
        } else {
            None
        })
        .highlight_style(Style::default().modifier(Modifier::ITALIC))
        .highlight_symbol(">>")
        .render(&mut f, rect);
}

fn draw_input_field<B: Backend>(mut f: &mut Frame<B>, rect: Rect, app: &App) {
    let command_input_style = if app.autoeval_mode {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };

    List::new(app.input_state.content_lines().iter().map(|l| Text::raw(l)))
        .block(make_default_block("Command", app.selected_area == UIArea::CommandInput).style(command_input_style))
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

fn draw_config<B: Backend>(mut f: &mut Frame<B>, rect: Rect, selected_area: UIArea, autoeval_mode: bool) {
    let config_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Percentage(50), Percentage(50)].as_ref())
        .split(rect);

    let immediate_eval_state = if autoeval_mode { "Active" } else { "Inactive" };
    Paragraph::new([Text::raw(immediate_eval_state)].iter())
        .block(make_default_block("Immediate eval", selected_area == UIArea::Config))
        .render(&mut f, config_chunks[0]);
}
