// SPDX-License-Identifier: GPL-3.0

//! The file types Postilla opens, shared by the command line, the file dialogs,
//! the vault sidebar and vault search. Extensions match case-insensitively.

use std::borrow::Cow;
use std::path::Path;

use cosmic::dialog::file_chooser::FileFilter;

/// Rendered as Markdown in the preview
pub const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdown", "mkd", "mdx"];
/// Shown as raw text in the preview
pub const TEXT_EXTENSIONS: &[&str] = &["txt", "text", "log"];
/// Shown as raw, highlighted text in the preview
pub const DATA_EXTENSIONS: &[&str] = &["json", "yaml", "yml", "toml"];

fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|e| e.to_str())
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    extension(path).is_some_and(|ext| extensions.iter().any(|e| ext.eq_ignore_ascii_case(e)))
}

fn all_extensions() -> impl Iterator<Item = &'static str> {
    MARKDOWN_EXTENSIONS
        .iter()
        .chain(TEXT_EXTENSIONS)
        .chain(DATA_EXTENSIONS)
        .copied()
}

/// Whether Postilla can open the file at `path`
pub fn is_supported(path: &Path) -> bool {
    has_extension(path, MARKDOWN_EXTENSIONS)
        || has_extension(path, TEXT_EXTENSIONS)
        || has_extension(path, DATA_EXTENSIONS)
}

/// Whether the preview renders the file as Markdown. Unsaved files (no path) are Markdown.
pub fn is_markdown(path: Option<&Path>) -> bool {
    path.is_none_or(|p| has_extension(p, MARKDOWN_EXTENSIONS))
}

/// Syntax token for the editor highlighter
pub fn highlight_token(path: Option<&Path>) -> String {
    match path.and_then(extension) {
        Some(ext) if !is_markdown(path) => ext.to_ascii_lowercase(),
        _ => String::from("md"),
    }
}

/// What the preview renders for `text`: Markdown as is, anything else as one
/// fenced code block so it shows the raw text
pub fn preview_source<'a>(path: Option<&Path>, text: &'a str) -> Cow<'a, str> {
    if is_markdown(path) {
        return Cow::Borrowed(text);
    }

    // the fence must be longer than any backtick run in the text
    let longest_run = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest_run.max(2) + 1);
    let token = highlight_token(path);
    let newline = if text.ends_with('\n') { "" } else { "\n" };

    Cow::Owned(format!("{fence}{token}\n{text}{newline}{fence}\n"))
}

/// File chooser filter for every supported type
pub fn file_filter() -> FileFilter {
    all_extensions().fold(FileFilter::new("Notes and Text Files"), |filter, ext| {
        filter
            .glob(&format!("*.{ext}"))
            .glob(&format!("*.{}", ext.to_ascii_uppercase()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_listed_extensions_in_any_case() {
        for name in [
            "a.md", "a.MD", "a.mdx", "a.txt", "a.Log", "a.json", "a.YML", "a.toml",
        ] {
            assert!(is_supported(Path::new(name)), "{name}");
        }
        for name in ["a.png", "a.rs", "README", "a.md.bak"] {
            assert!(!is_supported(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn markdown_tokens_use_the_markdown_syntax() {
        assert_eq!(highlight_token(Some(Path::new("a.mkd"))), "md");
        assert_eq!(highlight_token(None), "md");
        assert_eq!(highlight_token(Some(Path::new("a.JSON"))), "json");
    }

    #[test]
    fn non_markdown_preview_is_one_code_block() {
        let md = preview_source(Some(Path::new("a.md")), "# hi");
        assert_eq!(md, "# hi");

        let json = preview_source(Some(Path::new("a.json")), "{}");
        assert_eq!(json, "```json\n{}\n```\n");

        let fenced = preview_source(Some(Path::new("a.txt")), "````\nx\n");
        assert_eq!(fenced, "`````txt\n````\nx\n`````\n");
    }
}
