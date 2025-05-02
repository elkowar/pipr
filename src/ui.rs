use crate::app::command_list_window::CommandListState;
use crate::app::{App, WindowState};

use ansi_to_tui::IntoText;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::{
    backend::Backend,
    text::{Line, Span, Text},
    Frame, Terminal,
};
use std::io::{self, Write};
use Constraint::*;

use syntect::easy::HighlightLines;
use syntect::highlighting::{self, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use itertools::Itertools;
use lazy_static::lazy_static;

lazy_static! {
    static ref THEME_SET: ThemeSet = ThemeSet::load_defaults();
    static ref SYNTAX_SET: SyntaxSet = SyntaxSet::load_defaults_newlines();
    static ref THEME: &'static Theme = THEME_SET.themes.get("base16-ocean.dark").unwrap();
    static ref SH_SYNTAX: &'static SyntaxReference = SYNTAX_SET.find_syntax_by_extension("sh").unwrap();
    static ref PLAINTEXT_SYNTAX: &'static SyntaxReference = SYNTAX_SET.find_syntax_plain_text();
}

pub fn draw_app<B: Backend>(terminal: &mut Terminal<B>, mut app: &mut App) -> anyhow::Result<()> {
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
        let rect = Rect::new(0, 0, size.width, size.height);
        terminal.resize(rect)?; // this will redraw the whole screen
    }

    let mut input_field_rect = ratatui::layout::Rect::new(0, 0, 0, 0);
    terminal.draw(|mut f| {
        let root_rect = f.area();
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
                    // TODO render specific title
                    let options = opened_key_select_menu
                        .option_list_strings()
                        .map(|opt| ListItem::new(Span::raw(opt)))
                        .collect::<Vec<_>>();

                    f.render_widget(List::new(options).block(make_default_block("Open in", false)), root_chunks[0]);
                }

                input_field_rect = exec_chunks[0];

                draw_input_field(&mut f, input_field_rect, &mut app);

                if let Some(autocomplete_state) = &app.autocomplete_state {
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
                draw_outputs(
                    &mut f,
                    exec_chunks[2],
                    app.input_state.content_str() == app.last_executed_cmd,
                    app.is_processing_state,
                    &app.command_output,
                    &app.command_error,
                );

                let cursor_x = input_field_rect.x + 1 + app.input_state.displayed_cursor_column() as u16;
                let cursor_y = input_field_rect.y + 1 + app.input_state.cursor_line as u16;
                f.set_cursor_position((cursor_x, cursor_y));
            }
            WindowState::TextView(title, text) => {
                f.render_widget(
                    Paragraph::new(text.as_str()).block(make_default_block(title, true)),
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
            Paragraph::new("Help: F1"),
            Rect::new(root_rect.width - 10 as u16, root_rect.height as u16, 10, 1),
        );
    })?;

    Ok(())
}

fn draw_command_list(f: &mut Frame, rect: Rect, always_show_preview: bool, state: &CommandListState, title: &str) {
    let show_preview = always_show_preview || state.selected_entry().map(|e| e.lines().len() > 1) == Some(true);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Percentage(if show_preview { 60 } else { 100 }), Percentage(100)].as_ref())
        .split(rect);

    let items = state
        .list
        .iter()
        .map(|entry| entry.as_string().replace("\n", " ↵ "))
        .map(|entry| ListItem::new(Span::raw(entry)))
        .collect::<Vec<_>>();

    let mut list_state = ListState::default();
    list_state.select(state.selected_idx);

    let list_widget = List::new(items)
        .block(make_default_block(title, true))
        .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
        .highlight_symbol(">>");

    f.render_stateful_widget(list_widget, chunks[0], &mut list_state);

    if show_preview {
        if let Some(selected_content) = state.selected_entry() {
            f.render_widget(
                Paragraph::new(selected_content.as_string().as_str()).block(make_default_block("Preview", false)),
                chunks[1],
            );
        }
    }
}

fn draw_input_field(f: &mut Frame, rect: Rect, app: &mut App) {
    // TODO this is hideously inefficient
    //      also make themes configurable?
    //      also highlight errors if possible?

    let mut highlighter = HighlightLines::new(*SH_SYNTAX, &THEME);

    // cut off lines at the input field width, adding ...
    let lines: Vec<String> = app
        .input_state
        .content_lines()
        .iter()
        .map(|line| truncate_with_ellipsis(line.clone(), rect.width as usize))
        .collect_vec();

    let joined_lines = lines.join("\n");
    let styled_lines = if app.config.highlighting_enabled {
        LinesWithEndings::from(joined_lines.as_ref())
            .map(|line| {
                let Ok(result) = highlighter.highlight_line(line, &SYNTAX_SET) else {
                    return vec![Span::raw(line)];
                };
                result
                    .iter()
                    .map(|(style, part)| Span::styled(*part, highlight_style_to_ratatui_style(&style)))
                    .collect_vec()
            })
            .map(Line::from)
            .collect_vec()
    } else {
        lines.iter().map(Span::raw).map(Line::from).collect_vec()
    };

    let is_bookmarked = app.bookmarks.entries().contains(&app.input_state.content_to_commandentry());

    let input_block_title = format!(
        "Command{}{}{}{}",
        if is_bookmarked { " [Bookmarked]" } else { "" },
        if app.autoeval_mode { " [Autoeval]" } else { "" },
        if app.cached_command_part.is_some() { " [Caching]" } else { "" },
        if app.autoeval_mode && app.paranoid_history_mode {
            " [Paranoid]"
        } else {
            ""
        }
    );

    f.render_widget(
        Paragraph::new(Text::from(styled_lines)).block(make_default_block(&input_block_title, true)),
        rect,
    );
}

fn truncate_with_ellipsis(mut line: String, length: usize) -> String {
    if line.len() > length - 5 {
        line.truncate(length - 5);
        line.push_str("...");
    }
    line
}

fn draw_outputs(f: &mut Frame, rect: Rect, changed: bool, processing_state: Option<u8>, stdout: &str, stderr: &str) {
    let text = stdout.into_text().unwrap_or_else(|_| Text::raw(stdout));

    let output_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Percentage(if stderr.is_empty() { 100 } else { 50 }), Percentage(100)].as_ref())
        .split(rect);

    let stdout_title = format!(
        "Output{}{}",
        if changed { "" } else { " [+]" },
        display_processing_state(processing_state)
    );

    // TODO only render the amount of lines that is actually visible, or make it scrollable
    f.render_widget(
        Paragraph::new(text).block(make_default_block(&stdout_title, false)),
        output_chunks[0],
    );

    if !stderr.is_empty() {
        let stderr_text = stderr.into_text().unwrap_or_else(|_| Text::raw(stderr));
        f.render_widget(
            Paragraph::new(stderr_text).block(make_default_block("Stderr", false)),
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

    Block::default().title(Span::styled(title, title_style)).borders(Borders::ALL)
}

fn display_processing_state(state: Option<u8>) -> &'static str {
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

fn highlight_style_to_ratatui_style(style: &highlighting::Style) -> Style {
    let fg = style.foreground;
    Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b)).bg(Color::Reset)
}
