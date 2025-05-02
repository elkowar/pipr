use ansi_to_tui::IntoText;
use ratatui::{
    layout::{Constraint::Percentage, Direction, Layout, Rect},
    text::Text,
    widgets::Paragraph,
    Frame,
};

use crate::ui::{display_processing_state, make_default_block};

/// Draw command output and error sections
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `rect` - The area to render in
/// * `changed` - Whether the command has changed since last execution
/// * `processing_state` - Current processing animation state
/// * `stdout` - Standard output from the command
/// * `stderr` - Standard error from the command
pub fn draw_outputs(f: &mut Frame, rect: Rect, changed: bool, processing_state: Option<u8>, stdout: &str, stderr: &str) {
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
