use crate::app::App;
use itertools::Itertools;
use ratatui::{
    layout::Rect,
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;

use super::SH_SYNTAX;
use super::SYNTAX_SET;
use super::THEME;
use crate::ui::highlight_style_to_ratatui_style;
use crate::ui::{make_default_block, truncate_with_ellipsis};

/// Draw the input field for commands
pub fn draw_input_field(f: &mut Frame, rect: Rect, app: &mut App) {
    let mut highlighter = HighlightLines::new(*SH_SYNTAX, &THEME);

    // Cut off lines at the input field width, adding ...
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
                    .map(|(style, part)| Span::styled(*part, highlight_style_to_ratatui_style(style)))
                    .collect_vec()
            })
            .map(Line::from)
            .collect_vec()
    } else {
        lines.iter().map(Span::raw).map(Line::from).collect_vec()
    };

    let is_bookmarked = app.bookmarks.entries().contains(&app.input_state.content_to_commandentry());

    // Create descriptive title showing current modes
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
