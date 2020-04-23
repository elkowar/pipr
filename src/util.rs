/// returns the word at the given byte index.
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_word_under_cursor() {
        assert_eq!("abc def ghi".word_at_idx(5), Some("def"));
        assert_eq!("abc def ghi".word_at_idx(2), Some("abc"));
        assert_eq!("abc def ghi".word_at_idx(0), Some("abc"));
        assert_eq!("abc def ghi".word_at_idx(10), Some("ghi"));
        assert_eq!("".word_at_idx(0), None);
        assert_eq!("".word_at_idx(2), None);
        assert_eq!("abc".word_at_idx(0), Some("abc"));
        assert_eq!("abc".word_at_idx(3), Some("abc"));
        assert_eq!("abc     def ghi".word_at_idx(3), Some("abc"));
        assert_eq!("abc     def ghi".word_at_idx(4), None);
        assert_eq!("äää".word_at_idx(2), Some("äää"));
    }

    #[test]
    fn test_get_full_char_at() {
        assert_eq!("abc".get_full_char_at(0), Some("a"));
        assert_eq!("abc".get_full_char_at(1), Some("b"));
        assert_eq!("aääc".get_full_char_at(1), Some("ä"));
    }
}

pub trait StringExt {
    fn word_at_idx(&self, idx: usize) -> Option<&str>;
    fn get_full_char_at(&self, idx: usize) -> Option<&str>;
}

impl<T: AsRef<str>> StringExt for T {
    fn word_at_idx(&self, idx: usize) -> Option<&str> {
        let adjusted_cursor = {
            let hovered_char = self.get_full_char_at(idx);
            if (hovered_char == Some(" ") || hovered_char == None) && idx > 0 {
                idx - 1
            } else {
                idx
            }
        };

        let mut left_end = adjusted_cursor;
        while (self.get_full_char_at(left_end) != Some(" ") || self.as_ref().get(left_end..) == None) && left_end > 0 {
            left_end -= 1;
        }

        let mut right_end = adjusted_cursor;
        while (self.get_full_char_at(right_end) != Some(" ") || self.as_ref().get(..right_end) == None)
            && right_end < self.as_ref().len()
        {
            right_end += 1;
        }
        // don't keep if empty
        self.as_ref()
            .get(left_end..right_end)
            .map(|x| x.trim())
            .filter(|&word| word != "")
    }

    fn get_full_char_at(&self, idx: usize) -> Option<&str> {
        let line: &str = self.as_ref();
        let mut char_end = idx + 1;
        while line.get(idx..char_end) == None && line.len() >= char_end {
            char_end += 1;
        }
        line.get(idx..char_end)
    }
}
