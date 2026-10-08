// SPDX-License-Identifier: GPL-3.0

/// Statistics about a document
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DocStats {
    pub words: usize,
    pub chars: usize,
    pub lines: usize,
    /// Estimated reading time in minutes (at ~200 words per minute, min 1)
    pub reading_minutes: usize,
}

impl DocStats {
    pub fn compute(text: &str) -> Self {
        let words = text.split_whitespace().count();
        let chars = text.chars().filter(|c| !c.is_whitespace()).count();
        let lines = if text.is_empty() {
            0
        } else {
            text.lines().count()
        };
        let reading_minutes = if words == 0 { 0 } else { words.div_ceil(200) };

        Self {
            words,
            chars,
            lines,
            reading_minutes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_stats() {
        let s = DocStats::compute("hello world\nsecond line");
        assert_eq!(s.words, 4);
        assert_eq!(s.chars, 20);
        assert_eq!(s.lines, 2);
        assert_eq!(s.reading_minutes, 1);
        assert_eq!(DocStats::compute(""), DocStats::default());
    }
}
