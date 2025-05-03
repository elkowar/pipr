#[cfg(test)]
mod stringext_test {
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
            if (hovered_char == Some(" ") || hovered_char.is_none()) && idx > 0 {
                idx - 1
            } else {
                idx
            }
        };

        let mut left_end = adjusted_cursor;
        while (self.get_full_char_at(left_end) != Some(" ") || self.as_ref().get(left_end..).is_none()) && left_end > 0 {
            left_end -= 1;
        }

        let mut right_end = adjusted_cursor;
        while (self.get_full_char_at(right_end) != Some(" ") || self.as_ref().get(..right_end).is_none())
            && right_end < self.as_ref().len()
        {
            right_end += 1;
        }
        // don't keep if empty
        self.as_ref()
            .get(left_end..right_end)
            .map(|x| x.trim())
            .filter(|&word| !word.is_empty())
    }

    fn get_full_char_at(&self, idx: usize) -> Option<&str> {
        let line: &str = self.as_ref();
        let mut char_end = idx + 1;
        while line.get(idx..char_end).is_none() && line.len() >= char_end {
            char_end += 1;
        }
        line.get(idx..char_end)
    }
}

pub trait VecStringExt {
    fn split_strings_at_offset(&self, line_offset: usize, col_offset: usize) -> (Vec<String>, Vec<String>);
}

impl VecStringExt for Vec<String> {
    fn split_strings_at_offset(&self, line_offset: usize, col_offset: usize) -> (Vec<String>, Vec<String>) {
        // TODO this should be possible without allocations, returning Vec<&str>'s
        if self.is_empty() {
            return (Vec::new(), Vec::new());
        }

        if line_offset == 0 && col_offset == 0 {
            return (Vec::new(), self.clone());
        } else if line_offset >= self.len() - 1 && col_offset >= self.last().unwrap().len() {
            return (self.clone(), Vec::new());
        }
        let left_side = self
            .iter()
            .take(line_offset)
            .chain([self[line_offset][..col_offset].to_string()].iter())
            .cloned()
            .collect::<Vec<String>>();

        let right_side = [self[line_offset][col_offset..].to_string()]
            .iter()
            .chain(self[line_offset + 1..].iter())
            .cloned()
            .collect::<Vec<String>>();

        (left_side, right_side)
    }
}

#[cfg(test)]
mod vecstringext_test {
    use super::*;
    #[test]
    fn test_split_at_offset() {
        {
            let base_lines = vec!["".into(), "abcd".into(), "abcd".into(), "".into(), "abcd".into()];
            let (left_side, right_side) = base_lines.split_strings_at_offset(2, 2);
            assert_eq!(left_side, vec!["", "abcd", "ab"]);
            assert_eq!(right_side, vec!["cd", "", "abcd"]);
        }
        {
            let base_lines = vec!["abcd".into(), "abcd".into()];
            let (left_side, right_side) = base_lines.split_strings_at_offset(0, 2);
            assert_eq!(left_side, vec!["ab"]);
            assert_eq!(right_side, vec!["cd", "abcd"]);
        }
    }

    #[test]
    fn test_at_bounds() {
        {
            let base_lines = vec!["abcd".into(), "abcd".into(), "abcd".into()];
            let (left_side, right_side) = base_lines.split_strings_at_offset(0, 0);
            assert_eq!(left_side, Vec::new() as Vec<String>);
            assert_eq!(right_side, vec!["abcd", "abcd", "abcd"]);
        }
        {
            let base_lines = vec!["abcd".into(), "abcd".into(), "abcd".into()];
            let (left_side, right_side) = base_lines.split_strings_at_offset(2, 4);
            assert_eq!(left_side, vec!["abcd", "abcd", "abcd"]);
            assert_eq!(right_side, Vec::new() as Vec<String>);
        }
    }
}
