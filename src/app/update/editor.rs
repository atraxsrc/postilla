// SPDX-License-Identifier: GPL-3.0

use crate::app::core::editor::{EditorSearchState, EditorState};
use crate::app::core::utils::search::SearchAction;
use crate::app::core::utils::{self};
use crate::app::{
    AppModel, ClipboardAction, Message, State, editor_scrollable_id, preview_scrollable_id,
    search_input_id, text_editor_id,
};
use crate::config::BoolState;
use cosmic::iced::widget::scrollable::scroll_to;
use cosmic::prelude::*;
use cosmic::widget::text_editor::{Cursor, Position};
use widgets::text_editor;

impl AppModel {
    pub fn handle_edit(&mut self, action: text_editor::Action) -> Task<cosmic::Action<Message>> {
        let State::Ready {
            editor, preview, ..
        } = &mut self.state
        else {
            return Task::none();
        };

        let was_edit = action.is_edit();
        let cursor_before = editor.content.cursor().position;

        if let text_editor::Action::Edit(text_editor::Edit::Enter) = &action {
            editor.handle_list_continuation();
        } else if let text_editor::Action::Edit(text_editor::Edit::Insert('\t')) = &action {
            editor.handle_list_indent();
        } else {
            editor.content.perform(action);
        }

        preview.update_content(editor.path.as_deref(), editor.content.text().as_ref());

        if was_edit {
            editor.is_dirty = true;
            editor.push_history((cursor_before.line, cursor_before.column));
        }

        let sync_preview = self.config.scrollbar_sync == BoolState::Yes;
        let cursor_task = if was_edit {
            ensure_cursor_visible(editor, sync_preview)
        } else {
            Task::none()
        };

        let download = utils::images::download_images(
            &mut preview.markstate,
            &mut preview.images_in_progress,
            &editor.path,
        );

        let autosave = if was_edit {
            self.schedule_autosave()
        } else {
            Task::none()
        };

        Task::batch([download.chain(cursor_task), autosave])
    }

    pub fn handle_apply_formatting(
        &mut self,
        action: utils::SelectionAction,
    ) -> Task<cosmic::Action<Message>> {
        let task = self.apply_formatting_to_selection(action);
        let autosave = self.schedule_autosave();
        Task::batch([task, autosave])
    }

    /// Schedules an autosave of the current file if autosave is enabled, the file
    /// exists on disk and has unsaved changes. Every call restarts the timer.
    pub fn schedule_autosave(&mut self) -> Task<cosmic::Action<Message>> {
        let delay = self.config.autosave_delay;
        let State::Ready { editor, .. } = &self.state else {
            return Task::none();
        };

        if delay == 0 || editor.path.is_none() || !editor.is_dirty {
            return Task::none();
        }

        self.autosave_generation = self.autosave_generation.wrapping_add(1);
        let generation = self.autosave_generation;

        Task::perform(
            tokio::time::sleep(std::time::Duration::from_secs(u64::from(delay))),
            move |_| cosmic::action::app(Message::AutosaveTick(generation)),
        )
    }

    /// Autosave timer fired: save unless the timer was restarted since
    pub fn handle_autosave_tick(&mut self, generation: u64) -> Task<cosmic::Action<Message>> {
        if generation != self.autosave_generation || self.config.autosave_delay == 0 {
            return Task::none();
        }

        let State::Ready { editor, .. } = &self.state else {
            return Task::none();
        };

        if editor.path.is_none() || !editor.is_dirty {
            return Task::none();
        }

        self.silent_save = true;
        self.handle_save_file()
    }

    /// Cut / copy / paste / select all on the editor
    pub fn handle_clipboard(&mut self, action: ClipboardAction) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &mut self.state else {
            return Task::none();
        };

        match action {
            ClipboardAction::Copy => match editor.content.selection() {
                Some(selection) if !selection.is_empty() => self.clipboard_write(selection),
                _ => Task::none(),
            },
            ClipboardAction::Cut => {
                let Some(selection) = editor.content.selection() else {
                    return Task::none();
                };
                if selection.is_empty() {
                    return Task::none();
                }

                let write = self.clipboard_write(selection);
                let delete = self.handle_edit(text_editor::Action::Edit(text_editor::Edit::Delete));
                write.chain(delete)
            }
            ClipboardAction::Paste => self.handle_smart_paste(),
            ClipboardAction::SelectAll => {
                editor.content.perform(text_editor::Action::SelectAll);
                widgets::text_editor::focus(text_editor_id())
            }
        }
    }

    /// Text read from the toolkit clipboard; falls back to the system clipboard
    /// when the toolkit had nothing to offer.
    pub fn handle_pasted_text(&mut self, text: Option<String>) -> Task<cosmic::Action<Message>> {
        let text = match text {
            Some(text) if !text.is_empty() => Some(text),
            _ => self.clipboard_read().filter(|t| !t.is_empty()),
        };

        match text {
            Some(text) => self.handle_edit(text_editor::Action::Edit(text_editor::Edit::Paste(
                std::sync::Arc::new(text),
            ))),
            None => Task::none(),
        }
    }

    /// Moves the cursor to the start of the given line, scrolls to it and focuses the editor
    pub fn handle_jump_to_line(&mut self, line: usize) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &mut self.state else {
            return Task::none();
        };

        let line = line.min(editor.content.line_count().saturating_sub(1));
        editor.content.move_to(Cursor {
            position: Position { line, column: 0 },
            selection: None,
        });

        let sync_preview = self.config.scrollbar_sync == BoolState::Yes;
        let scroll = match editor.scroll.last_editor_viewport {
            Some(_) => ensure_cursor_visible(editor, sync_preview),
            None => Task::none(),
        };

        scroll.chain(widgets::text_editor::focus(text_editor_id()))
    }

    pub fn handle_paste_image(&mut self) -> Task<cosmic::Action<Message>> {
        let State::Ready {
            editor, preview, ..
        } = &mut self.state
        else {
            return Task::none();
        };

        let target_dir = match &editor.path {
            Some(path) => path
                .parent()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| self.config.vault_path()),
            None => self.config.vault_path(),
        };

        match utils::images::save_clipboard_image(&target_dir) {
            Ok(file_name) => {
                let cursor_before = editor.content.cursor().position;
                let selection = editor.content.selection().unwrap_or_default();
                let alt = if selection.is_empty() { "" } else { &selection };
                let image_tag = format!("![{alt}]({file_name})");

                editor
                    .content
                    .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                        std::sync::Arc::new(image_tag),
                    )));

                editor.is_dirty = true;
                editor.push_history((cursor_before.line, cursor_before.column));

                preview.update_content(editor.path.as_deref(), editor.content.text().as_ref());

                let sync_preview = self.config.scrollbar_sync == BoolState::Yes;
                let cursor_task = ensure_cursor_visible(editor, sync_preview);

                let download = utils::images::download_images(
                    &mut preview.markstate,
                    &mut preview.images_in_progress,
                    &editor.path,
                );
                let autosave = self.schedule_autosave();

                Task::batch([download.chain(cursor_task), autosave])
            }
            Err(err) => {
                eprintln!("Failed to paste image: {err}");
                self.handle_add_toast(utils::PostillaToast::new(crate::fl!(
                    "no-image-clipboard"
                )))
            }
        }
    }

    pub fn handle_smart_paste(&mut self) -> Task<cosmic::Action<Message>> {
        let State::Ready {
            editor, preview, ..
        } = &mut self.state
        else {
            return Task::none();
        };

        let target_dir = match &editor.path {
            Some(path) => path
                .parent()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| self.config.vault_path()),
            None => self.config.vault_path(),
        };

        // Try pasting as an image first
        if let Ok(file_name) = utils::images::save_clipboard_image(&target_dir) {
            let cursor_before = editor.content.cursor().position;
            let selection = editor.content.selection().unwrap_or_default();
            let alt = if selection.is_empty() { "" } else { &selection };
            let image_tag = format!("![{alt}]({file_name})");

            editor
                .content
                .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                    std::sync::Arc::new(image_tag),
                )));

            editor.is_dirty = true;
            editor.push_history((cursor_before.line, cursor_before.column));

            preview.update_content(editor.path.as_deref(), editor.content.text().as_ref());

            let sync_preview = self.config.scrollbar_sync == BoolState::Yes;
            let cursor_task = ensure_cursor_visible(editor, sync_preview);

            let download = utils::images::download_images(
                &mut preview.markstate,
                &mut preview.images_in_progress,
                &editor.path,
            );
            let autosave = self.schedule_autosave();

            return Task::batch([download.chain(cursor_task), autosave]);
        }

        // Plain text: the toolkit clipboard (the focused window's data device) is
        // the most reliable source, the system clipboard is tried if it comes
        // back empty, see `handle_pasted_text`.
        cosmic::iced::clipboard::read().map(|text| cosmic::action::app(Message::PastedText(text)))
    }

    pub fn handle_toggle_checkbox(
        &mut self,
        index: usize,
        checked: bool,
    ) -> Task<cosmic::Action<Message>> {
        let State::Ready {
            editor, preview, ..
        } = &mut self.state
        else {
            return Task::none();
        };

        let text = editor.content.text();
        let Some((line, column)) = find_task_marker(&text, index) else {
            return Task::none();
        };

        let cursor_before = editor.content.cursor().position;

        // select the single character between the brackets and replace it
        editor.content.move_to(Cursor {
            position: Position {
                line,
                column: column + 1,
            },
            selection: Some(Position { line, column }),
        });
        editor
            .content
            .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                std::sync::Arc::new(if checked { "x" } else { " " }.to_string()),
            )));

        // put the cursor back where the user had it
        editor.content.move_to(Cursor {
            position: cursor_before,
            selection: None,
        });

        editor.is_dirty = true;
        editor.push_history((cursor_before.line, cursor_before.column));

        preview.update_content(editor.path.as_deref(), editor.content.text().as_ref());

        self.schedule_autosave()
    }

    pub fn handle_undo(&mut self) -> Task<cosmic::Action<Message>> {
        let State::Ready {
            editor, preview, ..
        } = &mut self.state
        else {
            return Task::none();
        };

        editor.undo(preview);

        let download = utils::images::download_images(
            &mut preview.markstate,
            &mut preview.images_in_progress,
            &editor.path,
        );
        Task::batch([download, self.schedule_autosave()])
    }

    pub fn handle_redo(&mut self) -> Task<cosmic::Action<Message>> {
        let State::Ready {
            editor, preview, ..
        } = &mut self.state
        else {
            return Task::none();
        };

        editor.redo(preview);

        let download = utils::images::download_images(
            &mut preview.markstate,
            &mut preview.images_in_progress,
            &editor.path,
        );
        Task::batch([download, self.schedule_autosave()])
    }

    pub fn handle_search(&mut self, action: SearchAction) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &mut self.state else {
            return Task::none();
        };

        let sync_preview = self.config.scrollbar_sync == BoolState::Yes;

        match action {
            SearchAction::ToggleSearch => {
                editor.search.show_search_box = !editor.search.show_search_box;
                // clear state when closing
                if !editor.search.show_search_box {
                    editor.search = EditorSearchState::default();
                    widgets::text_editor::focus(text_editor_id())
                        .chain(ensure_cursor_visible(editor, sync_preview))
                } else {
                    cosmic::widget::text_input::focus(search_input_id())
                }
            }

            SearchAction::UpdateSearchValue(new_value) => {
                editor.search.search_value = new_value;
                editor.search.compute_matches(&editor.content.text());

                if let Some(idx) = editor.search.current_match_index {
                    editor.navigate_to_match(&editor.search.matches[idx].clone());
                    ensure_cursor_visible(editor, sync_preview)
                } else {
                    Task::none()
                }
            }

            SearchAction::ToggleRegex => {
                editor.search.use_regex = !editor.search.use_regex;
                editor.search.compute_matches(&editor.content.text());

                if let Some(idx) = editor.search.current_match_index {
                    editor.navigate_to_match(&editor.search.matches[idx].clone());
                    ensure_cursor_visible(editor, sync_preview)
                } else {
                    Task::none()
                }
            }

            SearchAction::NextResult => {
                if let Some(m) = editor.search.next_match().cloned() {
                    editor.navigate_to_match(&m);
                    ensure_cursor_visible(editor, sync_preview)
                } else {
                    Task::none()
                }
            }

            SearchAction::PrevResult => {
                if let Some(m) = editor.search.prev_match().cloned() {
                    editor.navigate_to_match(&m);
                    ensure_cursor_visible(editor, sync_preview)
                } else {
                    Task::none()
                }
            }

            SearchAction::FocusSearchField => cosmic::widget::text_input::focus(search_input_id()),
        }
    }
}

/// Scrolls the editor to keep the cursor visible.
fn ensure_cursor_visible(
    editor: &mut EditorState,
    sync_preview: bool,
) -> Task<cosmic::Action<Message>> {
    let Some(editor_vp) = editor.scroll.last_editor_viewport else {
        return Task::none();
    };

    let total_lines = editor.content.line_count().max(1);
    let cursor_line = editor.content.cursor().position.line;
    let content_height = editor_vp.content_bounds().height;
    let viewport_height = editor_vp.bounds().height;
    let line_height = content_height / total_lines as f32;
    let cursor_top = cursor_line as f32 * line_height;
    let cursor_bottom = cursor_top + line_height;
    let scroll_y = editor_vp.absolute_offset().y;
    let padding = line_height * 3.0;

    let new_editor_y = if cursor_top < scroll_y + padding {
        // cursor above visible area
        (cursor_top - padding).max(0.0)
    } else if cursor_bottom > scroll_y + viewport_height - padding {
        // cursor below visible area
        cursor_bottom + padding - viewport_height
    } else {
        // already visible, nothing to do
        return Task::none();
    };

    // scroll editor, marking it as programmatic so it isn't re-synced via on_scroll
    editor.scroll.pending_editor_scrolls += 1;
    let editor_task = scroll_to(editor_scrollable_id(), utils::scroll::abs(new_editor_y))
        .map(cosmic::action::app);

    // if sync is active, also scroll the preview proportionally
    if let Some(preview_vp) = editor.scroll.last_preview_viewport
        && sync_preview
    {
        let editor_scrollable = (content_height - viewport_height).max(0.0);
        let rel = if editor_scrollable > 0.0 {
            new_editor_y / editor_scrollable
        } else {
            0.0
        };
        let preview_scrollable =
            (preview_vp.content_bounds().height - preview_vp.bounds().height).max(0.0);
        let new_preview_y = (rel * preview_scrollable).max(0.0);

        editor.scroll.pending_preview_scrolls += 1;
        let preview_task = scroll_to(preview_scrollable_id(), utils::scroll::abs(new_preview_y))
            .map(cosmic::action::app);

        return editor_task.chain(preview_task);
    }

    editor_task
}

/// Finds the nth task list marker (`- [ ]`, `* [x]`, `1. [ ]`...) in the text,
/// skipping fenced code blocks. Returns the line and the column (in chars) of
/// the character between the brackets.
fn find_task_marker(text: &str, index: usize) -> Option<(usize, usize)> {
    let mut in_fence = false;
    let mut count = 0;

    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        if let Some(column) = task_marker_column(line) {
            if count == index {
                return Some((line_number, column));
            }
            count += 1;
        }
    }

    None
}

/// If the line is a task list item, returns the column (in chars) of the
/// character between the brackets.
fn task_marker_column(line: &str) -> Option<usize> {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    // indentation and blockquote markers
    while i < chars.len() && matches!(chars[i], ' ' | '\t' | '>') {
        i += 1;
    }

    // bullet (-, *, +) or ordered marker (1. or 1))
    if i < chars.len() && matches!(chars[i], '-' | '*' | '+') {
        i += 1;
    } else {
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        if i == start || i >= chars.len() || !matches!(chars[i], '.' | ')') {
            return None;
        }
        i += 1;
    }

    // at least one space after the marker
    let start = i;
    while i < chars.len() && matches!(chars[i], ' ' | '\t') {
        i += 1;
    }
    if i == start {
        return None;
    }

    // [ ], [x] or [X], followed by whitespace or the end of the line
    let is_task = i + 2 < chars.len()
        && chars[i] == '['
        && matches!(chars[i + 1], ' ' | 'x' | 'X')
        && chars[i + 2] == ']'
        && chars.get(i + 3).is_none_or(|c| c.is_whitespace());

    is_task.then_some(i + 1)
}
