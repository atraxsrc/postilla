// SPDX-License-Identifier: GPL-3.0

/// A heading of a markdown document
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// Heading level (1-6)
    pub level: u8,
    /// Heading text without the `#` markers
    pub text: String,
    /// Zero based line of the heading in the document
    pub line: usize,
}

/// Collects the ATX headings (`# Title`) of a document, skipping fenced code blocks.
pub fn headings(text: &str) -> Vec<Heading> {
    let mut result = Vec::new();
    let mut fence: Option<&str> = None;

    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();

        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }

        if trimmed.starts_with("```") {
            fence = Some("```");
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some("~~~");
            continue;
        }

        // up to 3 spaces of indentation are allowed for headings
        if line.len() - trimmed.len() > 3 {
            continue;
        }

        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if level == 0 || level > 6 {
            continue;
        }

        let rest = &trimmed[level..];
        if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
            continue;
        }

        let text = rest.trim().trim_end_matches('#').trim().to_string();
        result.push(Heading {
            level: level as u8,
            text,
            line: line_number,
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_headings_and_skips_fences() {
        let doc = "# Title\n\ntext\n## Sub ##\n```\n# not a heading\n```\n   ### Deep\n####### too deep\n#nospace";
        let h = headings(doc);
        assert_eq!(h.len(), 3);
        assert_eq!(h[0], Heading { level: 1, text: "Title".into(), line: 0 });
        assert_eq!(h[1], Heading { level: 2, text: "Sub".into(), line: 3 });
        assert_eq!(h[2], Heading { level: 3, text: "Deep".into(), line: 7 });
    }
}
