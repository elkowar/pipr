#[derive(Debug, Clone)]
pub struct Snippet {
    pub text: String,
    pub cursor_offset: usize,
}

impl Snippet {
    pub fn parse(s: &str) -> Snippet {
        Snippet {
            text: str::replace(s, "||", ""),
            cursor_offset: s.find("||").unwrap_or(s.len()),
        }
    }

    pub fn without_pipe(&self) -> &str {
        self.text.trim().trim_start_matches('|').trim()
    }
}
