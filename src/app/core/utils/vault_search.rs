// SPDX-License-Identifier: GPL-3.0

use std::path::{Path, PathBuf};

use grep::matcher::Matcher;
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{Searcher, sinks::UTF8};

/// Maximum number of results returned by a vault search
pub const MAX_RESULTS: usize = 300;
/// Maximum length (in chars) of a result snippet
const MAX_SNIPPET_CHARS: usize = 160;

/// Actions of the vault search context page
#[derive(Debug, Clone)]
pub enum VaultSearchAction {
    /// The query text changed
    UpdateQuery(String),
    /// Toggle regex mode
    ToggleRegex,
    /// Results of a search (generation, results)
    Results(u64, Vec<VaultSearchResult>),
    /// A search failed (generation, error)
    Failed(u64, String),
    /// Open the given file at the given (zero based) line
    Open(PathBuf, usize),
}

/// A line of a vault file matching the query
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultSearchResult {
    pub path: PathBuf,
    /// Zero based line number
    pub line: usize,
    /// The matching line, trimmed and truncated
    pub snippet: String,
}

/// State of the vault search context page
#[derive(Debug, Default)]
pub struct VaultSearchState {
    pub query: String,
    pub use_regex: bool,
    pub results: Vec<VaultSearchResult>,
    pub searching: bool,
    pub error: Option<String>,
    /// Incremented on every query change so stale results can be ignored
    pub generation: u64,
}

/// Searches every markdown / text file of `root` for `query`.
pub fn search_vault(root: &Path, query: &str, use_regex: bool) -> Result<Vec<VaultSearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let matcher = RegexMatcherBuilder::new()
        .fixed_strings(!use_regex)
        .case_insensitive(true)
        .build(query)
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    let mut searcher = Searcher::new();

    for entry in ignore::WalkBuilder::new(root).hidden(false).build() {
        if results.len() >= MAX_RESULTS {
            break;
        }

        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();
        let is_note = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("txt"));
        if !is_note {
            continue;
        }

        let path_buf = path.to_path_buf();
        let _ = searcher.search_path(
            &matcher,
            path,
            UTF8(|line_number, line| {
                if results.len() >= MAX_RESULTS {
                    return Ok(false);
                }
                if matcher.find(line.as_bytes()).ok().flatten().is_some() {
                    results.push(VaultSearchResult {
                        path: path_buf.clone(),
                        line: (line_number as usize).saturating_sub(1),
                        snippet: snippet(line),
                    });
                }
                Ok(true)
            }),
        );
    }

    Ok(results)
}

fn snippet(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.chars().count() > MAX_SNIPPET_CHARS {
        let cut: String = trimmed.chars().take(MAX_SNIPPET_CHARS).collect();
        format!("{cut}…")
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_matches_in_notes() {
        let dir = std::env::temp_dir().join(format!("cedilla-search-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("a.md"), "first line\nNeedle here\n").unwrap();
        std::fs::write(dir.join("b.txt"), "needle again").unwrap();
        std::fs::write(dir.join("c.png"), "needle in binary").unwrap();

        let mut results = search_vault(&dir, "needle", false).unwrap();
        results.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].line, 1);
        assert_eq!(results[0].snippet, "Needle here");
        assert_eq!(results[1].line, 0);

        assert!(search_vault(&dir, "", false).unwrap().is_empty());
        assert!(search_vault(&dir, "(", true).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
