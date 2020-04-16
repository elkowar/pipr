use std::fmt::{self, Display, Formatter};
#[derive(Debug, Clone, PartialEq)]
pub struct Snippet {
    pub text: String,
    pub cursor_offset: usize,
}

impl Snippet {
    pub fn new(text: String, cursor_offset: usize) -> Snippet {
        Snippet { text, cursor_offset }
    }

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

impl Display for Snippet {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text.trim())
    }
}

#[cfg(test)]
mod test {
    use super::Snippet;
    #[test]
    fn test_parsing() {
        assert_eq!(Snippet::parse("ab||c"), Snippet::new("abc".into(), 2));
        assert_eq!(Snippet::parse("abc"), Snippet::new("abc".into(), 3));
    }
    #[test]
    fn test_without_pipe() {
        let snippet = Snippet::new(" | abc".into(), 0);
        assert_eq!(snippet.without_pipe(), "abc");
        let snippet = Snippet::new("abc".into(), 0);
        assert_eq!(snippet.without_pipe(), "abc");
    }
}
