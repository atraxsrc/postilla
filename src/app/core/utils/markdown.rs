// SPDX-License-Identifier: GPL-3.0

use std::sync::Arc;

use cosmic::Task;
use widgets::text_editor;

use crate::app::{AppModel, Message, State};

/// Actions that can be performed on the current text selection
#[derive(Debug, Clone)]
pub enum SelectionAction {
    /// Convert selection to Heading 1 / Insert empty heading 1
    Heading1,
    /// Convert selection to Heading 2 / Insert empty heading 2
    Heading2,
    /// Convert selection to Heading 3 / Insert empty heading 3
    Heading3,
    /// Convert selection to Heading 4 / Insert empty heading 4
    Heading4,
    /// Convert selection to Heading 5 / Insert empty heading 5
    Heading5,
    /// Convert selection to Heading 6 / Insert empty heading 6
    Heading6,
    /// Convert selection to bold / Insert bold markers
    Bold,
    /// Convert selection to italic / Insert italic markers
    Italic,
    /// Convert selection to hyperlink / Insert hyperlink template
    Hyperlink,
    /// Convert selection to code
    Code,
    /// Convert selection to math
    Math,
    /// Convert selection to image / Insert image template
    Image,
    /// Convert selection to bulleted list / Insert list item
    BulletedList,
    /// Convert selection to numbered list / Insert numbered item
    NumberedList,
    /// Convert selection to checkbox list / Insert checkbox item
    CheckboxList,
    /// Add horizontal rule
    Rule,
    /// Convert selection to strikethrough / Insert strikethrough markers
    Strikethrough,
    /// Convert selection to a blockquote / Insert a quote marker
    Blockquote,
    /// Insert a table template
    Table,
    /// Insert the current date and time
    DateTime,
}

impl SelectionAction {
    pub fn is_line_action(&self) -> bool {
        matches!(
            &self,
            SelectionAction::BulletedList
                | SelectionAction::NumberedList
                | SelectionAction::CheckboxList
                | SelectionAction::Blockquote
        )
    }
}

impl AppModel {
    /// Apply formatting to the currently selected text in the editor
    pub fn apply_formatting_to_selection(
        &mut self,
        action: SelectionAction,
    ) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &mut self.state else {
            return Task::none();
        };

        let cursor_before = editor.content.cursor().position;
        let selection = editor.content.selection().unwrap_or_default();

        // for list actions with no selection, move to line start first
        let is_line_action = action.is_line_action();
        if selection.is_empty() && is_line_action {
            editor
                .content
                .perform(text_editor::Action::Move(text_editor::Motion::Home));
        };

        let formatted = format_selected_text(&selection, action);
        let formatted = formatted.trim_end_matches('\n').to_string();
        let formatted_len = formatted.chars().count();

        editor
            .content
            .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new(formatted),
            )));

        if selection.is_empty() && is_line_action {
            // if nothing was selected and we modify the full line keep the cursor at the end
            editor
                .content
                .perform(text_editor::Action::Move(text_editor::Motion::End));
        } else {
            for _ in 0..formatted_len {
                editor
                    .content
                    .perform(text_editor::Action::Move(text_editor::Motion::Left));
            }

            for _ in 0..formatted_len {
                editor
                    .content
                    .perform(text_editor::Action::Select(text_editor::Motion::Right));
            }
        }

        editor.push_history((cursor_before.line, cursor_before.column));
        editor.is_dirty = true;

        Task::none()
    }
}

/// Format the selected text based on the action
fn format_selected_text(selected_text: &str, action: SelectionAction) -> String {
    let is_empty = selected_text.is_empty();

    match action {
        SelectionAction::Heading1 => toggle_heading(selected_text, 1),
        SelectionAction::Heading2 => toggle_heading(selected_text, 2),
        SelectionAction::Heading3 => toggle_heading(selected_text, 3),
        SelectionAction::Heading4 => toggle_heading(selected_text, 4),
        SelectionAction::Heading5 => toggle_heading(selected_text, 5),
        SelectionAction::Heading6 => toggle_heading(selected_text, 6),

        SelectionAction::Bold => {
            if is_empty {
                "****".to_string()
            } else if selected_text.starts_with("**")
                && selected_text.ends_with("**")
                && selected_text.len() >= 4
            {
                selected_text[2..selected_text.len() - 2].to_string()
            } else {
                format!("**{}**", selected_text)
            }
        }

        SelectionAction::Italic => {
            if is_empty {
                "**".to_string()
            } else if is_italic(selected_text) {
                selected_text[1..selected_text.len() - 1].to_string()
            } else {
                format!("*{}*", selected_text)
            }
        }

        SelectionAction::Hyperlink => {
            if is_empty {
                "[](url)".to_string()
            } else if selected_text.starts_with('[') && selected_text.ends_with(')') {
                selected_text.to_string()
            } else {
                format!("[{}](url)", selected_text)
            }
        }

        SelectionAction::Code => {
            if is_empty {
                // cycle: nothing → inline → block → nothing
                "``".to_string()
            } else if selected_text.starts_with("```")
                && selected_text.ends_with("```")
                && selected_text.len() > 6
            {
                // code block → nothing (strip fences)
                let inner = &selected_text[3..selected_text.len() - 3];
                inner.trim_matches('\n').to_string()
            } else if selected_text.starts_with('`')
                && selected_text.ends_with('`')
                && selected_text.len() >= 2
            {
                // inline code → code block
                let inner = &selected_text[1..selected_text.len() - 1];
                format!("```\n{}\n```", inner)
            } else {
                // nothing → inline code
                format!("`{}`", selected_text)
            }
        }

        SelectionAction::Math => {
            if is_empty {
                "```typst\n\n```".to_string()
            } else if selected_text.starts_with("```typst") && selected_text.ends_with("```") {
                let inner = &selected_text["```typst".len()..selected_text.len() - 3];
                inner.trim_matches('\n').to_string()
            } else {
                format!("```typst\n{}\n```", selected_text)
            }
        }

        SelectionAction::Image => {
            if is_empty {
                "![](image-url)".to_string()
            } else if selected_text.starts_with("![") && selected_text.ends_with(')') {
                selected_text.to_string()
            } else {
                format!("![{}](image-url)", selected_text)
            }
        }

        SelectionAction::BulletedList => {
            if is_empty {
                "- ".to_string()
            } else if all_lines_have_prefix(selected_text, "- ") {
                remove_line_prefix(selected_text, "- ")
            } else {
                format_list(selected_text, "- ")
            }
        }

        SelectionAction::NumberedList => {
            if is_empty {
                "1. ".to_string()
            } else if all_lines_are_numbered(selected_text) {
                remove_numbered_list(selected_text)
            } else {
                format_numbered_list(selected_text)
            }
        }

        SelectionAction::CheckboxList => {
            if is_empty {
                "- [ ] ".to_string()
            } else if all_lines_have_prefix(selected_text, "- [ ] ")
                || all_lines_have_prefix(selected_text, "- [x] ")
            {
                remove_line_prefix(
                    remove_line_prefix(selected_text, "- [ ] ").as_str(),
                    "- [x] ",
                )
            } else {
                format_list(selected_text, "- [ ] ")
            }
        }

        SelectionAction::Rule => "---".to_string(),

        SelectionAction::Strikethrough => {
            if is_empty {
                "~~~~".to_string()
            } else if selected_text.starts_with("~~")
                && selected_text.ends_with("~~")
                && selected_text.len() >= 4
            {
                selected_text[2..selected_text.len() - 2].to_string()
            } else {
                format!("~~{}~~", selected_text)
            }
        }

        SelectionAction::Blockquote => {
            if is_empty {
                "> ".to_string()
            } else if all_lines_have_prefix(selected_text, "> ") {
                remove_line_prefix(selected_text, "> ")
            } else {
                format_list(selected_text, "> ")
            }
        }

        SelectionAction::Table => {
            if is_empty {
                "| Column 1 | Column 2 | Column 3 |\n| --- | --- | --- |\n|  |  |  |\n".to_string()
            } else {
                // every line becomes a row, cells separated by commas or tabs
                let rows: Vec<Vec<&str>> = selected_text
                    .lines()
                    .map(|line| {
                        line.split(['\t', ','])
                            .map(str::trim)
                            .collect::<Vec<_>>()
                    })
                    .collect();
                let columns = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);

                let mut out = String::new();
                for (i, row) in rows.iter().enumerate() {
                    out.push('|');
                    for c in 0..columns {
                        out.push(' ');
                        out.push_str(row.get(c).copied().unwrap_or(""));
                        out.push_str(" |");
                    }
                    out.push('\n');
                    if i == 0 {
                        out.push('|');
                        for _ in 0..columns {
                            out.push_str(" --- |");
                        }
                        out.push('\n');
                    }
                }
                out
            }
        }

        SelectionAction::DateTime => current_date_time(),
    }
}

/// The current local date and time, e.g. `2026-10-08 14:05`
pub fn current_date_time() -> String {
    jiff::Zoned::now().strftime("%Y-%m-%d %H:%M").to_string()
}

fn toggle_heading(text: &str, level: usize) -> String {
    let hashes = "#".repeat(level);

    if text.is_empty() {
        return format!("{} ", hashes);
    }

    let trimmed = text.trim();

    // if already this heading level
    let this_prefix = format!("{} ", hashes);
    if trimmed.starts_with(&this_prefix) {
        return trimmed[this_prefix.len()..].to_string();
    }

    // check if it's a different heading level, strip it and apply new one
    let without_existing = strip_heading(trimmed);
    format!("{} {}", hashes, without_existing.trim())
}

/// Strip any leading heading markers from text
fn strip_heading(text: &str) -> &str {
    let mut chars = text.chars().peekable();
    let mut count = 0;
    while chars.peek() == Some(&'#') {
        chars.next();
        count += 1;
    }
    if count > 0 && chars.peek() == Some(&' ') {
        &text[count + 1..]
    } else {
        text
    }
}

/// Returns true if text is italic but not bold
fn is_italic(text: &str) -> bool {
    text.starts_with('*') && text.ends_with('*') && text.len() >= 2 && !text.starts_with("**")
}

/// Returns true if every line starts with the given prefix
fn all_lines_have_prefix(text: &str, prefix: &str) -> bool {
    text.lines().all(|line| line.starts_with(prefix))
}

/// Remove a prefix from every line
fn remove_line_prefix(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| line.strip_prefix(prefix).unwrap_or(line).to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns true if every line looks like a numbered list item
fn all_lines_are_numbered(text: &str) -> bool {
    text.lines().enumerate().all(|(i, line)| {
        let prefix = format!("{}. ", i + 1);
        line.starts_with(&prefix)
    })
}

/// Remove numbered list formatting from every line
fn remove_numbered_list(text: &str) -> String {
    text.lines()
        .enumerate()
        .map(|(i, line)| {
            let prefix = format!("{}. ", i + 1);
            if line.starts_with(&prefix) {
                line[prefix.len()..].to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_list(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| {
            if line.trim().is_empty() {
                prefix.to_string()
            } else {
                format!("{}{}", prefix, line.trim())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_numbered_list(text: &str) -> String {
    text.lines()
        .enumerate()
        .map(|(i, line)| {
            if line.trim().is_empty() {
                format!("{}. ", i + 1)
            } else {
                format!("{}. {}", i + 1, line.trim())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn is_list_line(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    strip_numbered_prefix(trimmed).is_some()
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || trimmed.starts_with("- [ ] ")
        || trimmed.starts_with("- [x] ")
}

/// If the current line is a list item, returns the prefix to continue with on the next line.
/// Returns Some("") if the line is an empty list item (break  out of the list).
/// Returns None if not a list line.
pub fn get_list_continuation(content: &text_editor::Content) -> Option<String> {
    let cursor_line = content.cursor().position.line;
    let line = content.line(cursor_line)?.text;

    let indent = leading_indent(&line);
    let trimmed = &line[indent.len()..];

    // Numbered list
    if let Some(rest) = strip_numbered_prefix(trimmed) {
        if rest.trim().is_empty() {
            return Some(String::new()); // empty item, break out
        }
        // find current number and increment
        let num = current_list_number(trimmed)?;
        return Some(format!("{}. ", num + 1));
    }

    // Checkbox list (before bullet check since it also starts with "- ")
    for prefix in &["- [ ] ", "- [x] "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if rest.trim().is_empty() {
                return Some(String::new());
            }
            return Some("- [ ] ".to_string()); // insert unchecked
        }
    }

    // Bullet list
    for prefix in &["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if rest.trim().is_empty() {
                return Some(String::new());
            }
            return Some(prefix.to_string());
        }
    }

    None
}

fn leading_indent(line: &str) -> &str {
    let trimmed = line.trim_start_matches(' ');
    &line[..line.len() - trimmed.len()]
}

fn strip_numbered_prefix(line: &str) -> Option<&str> {
    let dot_pos = line.find(". ")?;
    let num_part = &line[..dot_pos];
    if num_part.chars().all(|c| c.is_ascii_digit()) && !num_part.is_empty() {
        Some(&line[dot_pos + 2..])
    } else {
        None
    }
}

fn current_list_number(line: &str) -> Option<usize> {
    let dot_pos = line.find(". ")?;
    line[..dot_pos].parse().ok()
}
