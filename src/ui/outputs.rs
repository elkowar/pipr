use ansi_to_tui::IntoText;
use ratatui::{
    layout::{Constraint::Percentage, Direction, Layout, Rect},
    text::Text,
    widgets::Paragraph,
    Frame,
};

use crate::ui::{display_processing_state, make_default_block};

/// Draw command output and error sections
pub fn draw_outputs(f: &mut Frame, rect: Rect, changed: bool, processing_state: Option<u8>, stdout: &str, stderr: &str) {
    let text = stdout.into_text().unwrap_or_else(|_| Text::raw(stdout));

    let stdout_title = format!(
        "Output{}{}",
        if changed { "" } else { " [+]" },
        display_processing_state(processing_state)
    );

    let [stdout_chunk, stderr_chunk] = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if stderr.is_empty() {
            [Percentage(100), Percentage(0)].as_ref()
        } else {
            [Percentage(50), Percentage(50)].as_ref()
        })
        .areas(rect);

    f.render_widget(
        Paragraph::new(text).block(make_default_block(&stdout_title, false)),
        stdout_chunk,
    );

    if !stderr.is_empty() {
        let stderr_text = stderr.into_text().unwrap_or_else(|_| Text::raw(stderr));
        f.render_widget(
            Paragraph::new(stderr_text).block(make_default_block("Stderr", false)),
            stderr_chunk,
        );
    }
}
