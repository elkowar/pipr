use crate::app::command_list_window::CommandListState;
use crate::app::*;
use ansi_parser::AnsiParser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    borrow::Cow,
    io::{self, Write},
};
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use tui::{
    backend::Backend,
    text::{Span, Spans, Text},
    Frame, Terminal,
};
use Constraint::*;

use syntect::easy::HighlightLines;
use syntect::highlighting::{self, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use util::VecStringExt;

lazy_static! {
    static ref THEME_SET: ThemeSet = ThemeSet::load_defaults();
    static ref SYNTAX_SET: SyntaxSet = SyntaxSet::load_defaults_newlines();
    static ref THEME: &'static Theme = THEME_SET.themes.get("base16-ocean.dark").unwrap();
    static ref SH_SYNTAX: &'static SyntaxReference = SYNTAX_SET.find_syntax_by_extension("sh").unwrap();
    static ref PLAINTEXT_SYNTAX: &'static SyntaxReference = SYNTAX_SET.find_syntax_plain_text();
}

pub fn draw_app<B: Backend>(terminal: &mut Terminal<B>, mut app: &mut App) -> Result<(), failure::Error> {
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
                    // TODO render specific title
                    let options = opened_key_select_menu
                        .option_list_strings()
                        .map(|opt| ListItem::new(Span::raw(opt)))
                        .collect_vec();

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
                            .collect_vec(),
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
                f.set_cursor(cursor_x, cursor_y);
            }
            WindowState::TextView(title, text) => {
                f.render_widget(
                    Paragraph::new(text.as_ref()).block(make_default_block(title, true)),
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

fn draw_command_list<B: Backend>(f: &mut Frame<B>, rect: Rect, always_show_preview: bool, state: &CommandListState, title: &str) {
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
        .collect_vec();

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

fn draw_input_field<B: Backend>(f: &mut Frame<B>, rect: Rect, app: &mut App) {
    // TODO this is hideously inefficient
    //      also make themes configurable?
    //      also highlight errors if possible?

    let mut highlighter = HighlightLines::new(*SH_SYNTAX, &THEME);

    let lines = if app.cached_command_part.is_none() {
        app.input_state
            .content_lines()
            .iter()
            .map(|line| {
                let mut line = line.clone();
                if line.len() > rect.width as usize - 5 {
                    line.truncate(rect.width as usize - 5);
                    line.push_str("...");
                }
                line
            })
            .collect::<Vec<String>>()
    } else {
        app.input_state.content_lines().clone()
    };

    let (cached_part, non_cached_part) = match app.cached_command_part {
        Some(CachedCommandPart { end_line, end_col, .. }) => lines.split_strings_at_offset(end_line, end_col),
        _ => (Vec::new(), lines),
    };
    let (cached_part, non_cached_part) = (cached_part.join("\n"), non_cached_part.join("\n"));

    let cached_part_styled = vec![Span::styled(
        Cow::Borrowed(cached_part.as_ref()),
        Style::default().bg(Color::DarkGray).fg(Color::White),
    )];

    let mut non_cached_part_styled = if app.config.highlighting_enabled {
        LinesWithEndings::from(&non_cached_part)
            .flat_map(|line| highlighter.highlight(line, &SYNTAX_SET))
            .map(|(style, part)| Span::styled(Cow::Borrowed(part), highlight_style_to_tui_style(&style)))
            .collect::<Vec<Span>>()
    } else {
        vec![Span::raw(non_cached_part)]
    };

    let mut full_styled = cached_part_styled;
    full_styled.append(&mut non_cached_part_styled);

    let is_bookmarked = app.bookmarks.entries().contains(&app.input_state.content_to_commandentry());

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
        Paragraph::new(Spans::from(full_styled)).block(make_default_block(&input_block_title, true)),
        rect,
    );
}

fn apply_graphics_mode_to_style(style: &mut Style, modes: &[u32]) {
    fn ansi_to_color(bright: bool, n: u32) -> Color {
        match (bright, n) {
            (false, 0) => Color::Black,
            (false, 1) => Color::Red,
            (false, 2) => Color::Green,
            (false, 3) => Color::Yellow,
            (false, 4) => Color::Blue,
            (false, 5) => Color::Magenta,
            (false, 6) => Color::Cyan,
            (false, 7) => Color::Gray,
            (true, 0) => Color::DarkGray,
            (true, 1) => Color::LightRed,
            (true, 2) => Color::LightGreen,
            (true, 3) => Color::LightYellow,
            (true, 4) => Color::LightBlue,
            (true, 5) => Color::LightMagenta,
            (true, 6) => Color::LightCyan,
            (true, 7) => Color::White,
            _ => Color::White,
        }
    }

    *style = match modes {
        [] | [0] => Style::default(),
        [1] => style.add_modifier(Modifier::BOLD),
        [3] => style.add_modifier(Modifier::ITALIC),
        [4] => style.add_modifier(Modifier::UNDERLINED),
        [5] => style.add_modifier(Modifier::SLOW_BLINK),
        [7] => style.add_modifier(Modifier::REVERSED),
        [8] => style.add_modifier(Modifier::HIDDEN),
        [9] => style.add_modifier(Modifier::CROSSED_OUT),
        [n @ 30..=37] => style.fg(ansi_to_color(false, n - 30)),
        [n @ 40..=47] => style.bg(ansi_to_color(false, n - 40)),
        [n @ 90..=97] => style.fg(ansi_to_color(true, n - 90)),
        [n @ 100..=107] => style.bg(ansi_to_color(true, n - 100)),
        [38, 5, n] => style.fg(Color::Indexed(*n as u8)),
        [48, 5, n] => style.bg(Color::Indexed(*n as u8)),
        [38, 2, r, g, b] => style.fg(Color::Rgb(*r as u8, *g as u8, *b as u8)),
        [48, 2, r, g, b] => style.bg(Color::Rgb(*r as u8, *g as u8, *b as u8)),
        _ => *style,
    };
}

fn draw_outputs<B: Backend>(
    f: &mut Frame<B>,
    rect: Rect,
    changed: bool,
    processing_state: Option<u8>,
    stdout: &str,
    stderr: &str,
) {
    let mut current_style = Style::default();
    let text_iter = stdout.lines().map(|line| {
        let spans = line
            .ansi_parse()
            .filter_map(|seq| match seq {
                ansi_parser::Output::TextBlock(text) => Some(Span::styled(Cow::from(text), current_style.clone())),
                ansi_parser::Output::Escape(ansi_parser::AnsiSequence::SetGraphicsMode(modes)) => {
                    apply_graphics_mode_to_style(&mut current_style, &modes);
                    None
                }
                _ => None,
            })
            .collect_vec();
        Spans::from(spans)
    });

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
        Paragraph::new(Text::from(text_iter.collect_vec())).block(make_default_block(&stdout_title, false)),
        output_chunks[0],
    );

    if !stderr.is_empty() {
        f.render_widget(
            Paragraph::new(stderr).block(make_default_block("Stderr", false)),
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

fn highlight_style_to_tui_style(style: &highlighting::Style) -> Style {
    let fg = style.foreground;
    Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b)).bg(Color::Reset)
}
