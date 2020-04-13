#[derive(Debug, Clone, PartialEq)]
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

#[cfg(test)]
mod test {
    use super::Snippet;
    #[test]
    fn test_parsing() {
        assert_eq!(
            Snippet::parse("ab||c"),
            Snippet {
                text: "abc".into(),
                cursor_offset: 2
            }
        );
        assert_eq!(
            Snippet::parse("abc"),
            Snippet {
                text: "abc".into(),
                cursor_offset: 3
            }
        )
    }
    #[test]
    fn test_without_pipe() {
        let snippet = Snippet {
            text: " | abc".into(),
            cursor_offset: 0,
        };
        assert_eq!(snippet.without_pipe(), "abc");
        let snippet = Snippet {
            text: "abc".into(),
            cursor_offset: 0,
        };
        assert_eq!(snippet.without_pipe(), "abc");
    }
}
