//! Predefined text snippets including cursor positioning information
use std::fmt::{self, Display, Formatter};

/// Text snippet with cursor positioning information.
#[derive(Debug, Clone, PartialEq)]
pub struct Snippet {
    /// The text content
    pub text: String,
    /// Position where cursor should be placed
    pub cursor_offset: usize,
}

impl Snippet {
    /// Creates a new Snippet with given text and cursor offset.
    #[cfg(test)]
    pub fn new(text: String, cursor_offset: usize) -> Snippet {
        Snippet { text, cursor_offset }
    }

    /// Parses a string into a Snippet, removing "||" marker and setting cursor position.
    pub fn parse(s: &str) -> Snippet {
        Snippet {
            text: str::replace(s, "||", ""),
            cursor_offset: s.find("||").unwrap_or(s.len()),
        }
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
}
