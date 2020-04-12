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

pub fn draw_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<(), failure::Error> {
    let mut input_field_rect = tui::layout::Rect::new(0, 0, 0, 0);
    terminal.draw(|mut f| {
        let root_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Percentage(if app.bookmarks_visible { 30 } else { 0 }), Percentage(80)].as_ref())
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

        draw_bookmark_list(&mut f, root_chunks[0], app.selected_area == UIArea::BookmarkList, &app);
        draw_input_field(&mut f, input_field_rect, app.selected_area == UIArea::CommandInput, &app);
        draw_outputs(&mut f, exec_chunks[2], &app.command_output, &app.command_error);
        draw_shortcuts(&mut f, exec_chunks[1], app.autoeval_mode);
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

fn draw_bookmark_list<B: Backend>(mut f: &mut Frame<B>, rect: Rect, is_focused: bool, app: &App) {
    SelectableList::default()
        .block(make_default_block("Bookmarks", is_focused))
        .items(app.bookmarks.as_strings().as_slice())
        .select(if is_focused { app.selected_bookmark_idx } else { None })
        .highlight_style(Style::default().modifier(Modifier::ITALIC))
        .highlight_symbol(">>")
        .render(&mut f, rect);
}

fn draw_input_field<B: Backend>(mut f: &mut Frame<B>, rect: Rect, is_focused: bool, app: &App) {
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
            is_focused,
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

fn draw_shortcuts<B: Backend>(mut f: &mut Frame<B>, rect: Rect, autoeval_mode: bool) {
    let immediate_eval_state = if autoeval_mode { "Active" } else { "Inactive" };
    let mappings = vec![
        format!("F1: Toggle autoeval ({})", immediate_eval_state),
        "Ctrl+B: Show/hide bookmarks".into(),
        "Ctrl+S: Toggle bookmark".into(),
    ];
    let mappings = mappings.join(" | ");
    Paragraph::new([Text::raw(mappings)].iter())
        .block(make_default_block("Immediate eval", false))
        .render(&mut f, rect);
}
