// SPDX-License-Identifier: GPL-3.0

use crate::app::core::editor::{EditorScrollState, EditorSearchState, EditorState};
use crate::app::core::history::HistoryState;
use crate::app::core::preview::MarkdownPreview;
use crate::app::core::utils::{self, PostillaToast};
use crate::app::{AppModel, Message, State, editor_scrollable_id, preview_scrollable_id};
use crate::app::{DiscardChangesAction, create_default_panes};
use crate::config::{BoolState, ViewMode};
use crate::file_types;
use cosmic::iced::widget::scrollable::scroll_to;
use cosmic::prelude::*;
use frostmark::MarkState;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use widgets::text_editor;

impl AppModel {
    pub fn handle_startup(
        &mut self,
        open_with_file: Option<PathBuf>,
    ) -> Task<cosmic::Action<Message>> {
        let panes = create_default_panes();
        let view_mode = self.config.last_view_mode;

        let path = open_with_file.or_else(|| match self.config.open_last_file {
            BoolState::Yes => self.config.last_open_file.clone(),
            BoolState::No => None,
        });

        if let Some(p) = path
            && p.exists()
        {
            // store parent directory of selected file
            self.selected_nav_path = p.parent().map(|p| p.to_path_buf());

            return Task::perform(utils::files::load_file(p), |res| {
                cosmic::action::app(Message::OpenFile(res))
            })
            .chain(Task::done(cosmic::action::app(Message::SetViewMode(view_mode))));
        }

        self.state = State::Ready {
            editor: EditorState {
                path: None,
                content: text_editor::Content::new(),
                is_dirty: true,
                history: HistoryState::default(),
                scroll: EditorScrollState::default(),
                search: EditorSearchState::default(),
                ignore_next_external_change: false,
            },
            preview: MarkdownPreview {
                markstate: MarkState::with_html_and_markdown(""),
                images: HashMap::new(),
                svgs: HashMap::new(),
                images_in_progress: HashSet::new(),
            },
            panes,
            view_mode,
        };
        Task::none()
    }

    pub fn handle_new_file(&mut self) -> Task<cosmic::Action<Message>> {
        let panes = get_previous_pane_state(self);
        let view_mode = get_previous_view_mode(self);

        self.state = State::Ready {
            editor: EditorState {
                path: None,
                content: text_editor::Content::new(),
                is_dirty: true,
                history: HistoryState::default(),
                scroll: EditorScrollState {
                    // pre-absorb the programmatic resets we're about to fire
                    pending_editor_scrolls: 1,
                    pending_preview_scrolls: 1,
                    ..EditorScrollState::default()
                },
                search: EditorSearchState::default(),
                ignore_next_external_change: false,
            },
            preview: MarkdownPreview {
                markstate: MarkState::with_html_and_markdown(""),
                images: HashMap::new(),
                svgs: HashMap::new(),
                images_in_progress: HashSet::new(),
            },
            panes,
            view_mode,
        };

        Task::batch([
            scroll_to(editor_scrollable_id(), crate::app::utils::scroll::abs(0.0))
                .map(cosmic::action::app),
            scroll_to(preview_scrollable_id(), crate::app::utils::scroll::abs(0.0))
                .map(cosmic::action::app),
        ])
    }

    pub fn handle_new_vault_file(&mut self, file_name: String) -> Task<cosmic::Action<Message>> {
        let dir = self.selected_directory();

        let file_name = file_name.trim();
        if file_name.is_empty() {
            return Task::none();
        }

        // "todo.md" or "notes.txt" are kept as typed, anything else becomes a Markdown note
        let (stem, ext) = if crate::file_types::is_supported(Path::new(file_name)) {
            let path = Path::new(file_name);
            let part = |s: Option<&std::ffi::OsStr>| {
                s.map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            };
            (part(path.file_stem()), part(path.extension()))
        } else {
            (file_name.to_string(), String::from("md"))
        };

        // find a name that doesn't already exist
        let file_path = {
            let base = dir.join(format!("{stem}.{ext}"));
            if !base.exists() {
                base
            } else {
                let mut i = 1;
                loop {
                    let candidate = dir.join(format!("{stem}-{i}.{ext}"));
                    if !candidate.exists() {
                        break candidate;
                    }
                    i += 1;
                }
            }
        };

        // create the file on disk
        if let Err(e) = std::fs::write(&file_path, "") {
            return self.handle_add_toast(PostillaToast::new(e));
        }

        self.insert_file_node(&file_path, &dir);

        let panes = get_previous_pane_state(self);
        let view_mode = get_previous_view_mode(self);

        self.state = State::Ready {
            editor: EditorState {
                path: Some(file_path),
                content: text_editor::Content::new(),
                is_dirty: true,
                history: HistoryState::default(),
                scroll: EditorScrollState {
                    // pre-absorb the programmatic resets we're about to fire
                    pending_editor_scrolls: 1,
                    pending_preview_scrolls: 1,
                    ..EditorScrollState::default()
                },
                search: EditorSearchState::default(),
                ignore_next_external_change: false,
            },
            preview: MarkdownPreview {
                markstate: MarkState::with_html_and_markdown(""),
                images: HashMap::new(),
                svgs: HashMap::new(),
                images_in_progress: HashSet::new(),
            },
            panes,
            view_mode,
        };

        Task::batch([
            scroll_to(editor_scrollable_id(), crate::app::utils::scroll::abs(0.0))
                .map(cosmic::action::app),
            scroll_to(preview_scrollable_id(), crate::app::utils::scroll::abs(0.0))
                .map(cosmic::action::app),
        ])
    }

    pub fn handle_new_vault_folder(
        &mut self,
        folder_name: String,
    ) -> Task<cosmic::Action<Message>> {
        let dir = self.selected_directory();

        let folder_path = {
            let base = dir.join(&folder_name);
            if !base.exists() {
                base
            } else {
                let mut i = 1;
                loop {
                    let candidate = dir.join(format!("{}-{}", folder_name, i));
                    if !candidate.exists() {
                        break candidate;
                    }
                    i += 1;
                }
            }
        };

        // create the folder on disk
        if let Err(e) = std::fs::create_dir(&folder_path) {
            return self.handle_add_toast(PostillaToast::new(e));
        }

        // insert folder to navbar
        self.insert_folder_node(&folder_path, &dir);

        Task::none()
    }

    pub fn handle_save_file(&mut self) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &mut self.state else {
            return Task::none();
        };

        if !editor.is_dirty {
            return Task::none();
        }

        let content = editor.content.text();
        let path = editor.path.clone();
        let vault_path = self.config.vault_path.clone();
        editor.ignore_next_external_change = true;

        Task::perform(
            async move {
                match path {
                    // We're editing an alreaday existing file
                    Some(path) => Some(utils::files::save_file(path, content).await),
                    // We want to save a new file
                    None => match utils::files::open_markdown_file_saver(vault_path).await {
                        Some(path) => Some(utils::files::save_file(path.into(), content).await),
                        // Error selecting where to save the file
                        None => None,
                    },
                }
            },
            |res| cosmic::action::app(Message::FileSaved(res)),
        )
    }

    /// "Save" in the unsaved changes dialog: save first, then carry on with
    /// closing the app or opening the other file.
    pub fn handle_save_then_continue(
        &mut self,
        action: DiscardChangesAction,
    ) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &self.state else {
            return Task::none();
        };

        if !editor.is_dirty {
            return self.handle_discard_changes(action);
        }

        self.pending_after_save = Some(action);
        self.handle_save_file()
    }

    pub fn handle_open_file(
        &mut self,
        result: Result<(PathBuf, Arc<String>), anywho::Error>,
    ) -> Task<cosmic::Action<Message>> {
        match result {
            Ok((path, content)) => {
                // store parent directory of selected file in nav_path only if path is inside vault
                let vault_path = self.config.vault_path();
                if path.starts_with(&vault_path) {
                    self.selected_nav_path = path.parent().map(|p| p.to_path_buf());
                }

                let panes = get_previous_pane_state(self);
                let view_mode = get_previous_view_mode(self);

                let markstate = MarkState::with_html_and_markdown(&file_types::preview_source(
                    Some(&path),
                    content.as_ref(),
                ));
                let images_in_progress = HashSet::new();

                self.state = State::Ready {
                    editor: EditorState {
                        path: Some(path),
                        content: text_editor::Content::with_text(content.as_ref()),
                        is_dirty: false,
                        history: HistoryState::new_with_content(content.to_string()),
                        scroll: EditorScrollState {
                            // pre-absorb the programmatic resets we're about to fire
                            pending_editor_scrolls: 1,
                            pending_preview_scrolls: 1,
                            ..EditorScrollState::default()
                        },
                        search: EditorSearchState::default(),
                        ignore_next_external_change: false,
                    },
                    preview: MarkdownPreview {
                        markstate,
                        images: HashMap::new(),
                        svgs: HashMap::new(),
                        images_in_progress,
                    },
                    panes,
                    view_mode,
                };

                let reset_editor =
                    scroll_to(editor_scrollable_id(), crate::app::utils::scroll::abs(0.0))
                        .map(cosmic::action::app);

                let reset_preview =
                    scroll_to(preview_scrollable_id(), crate::app::utils::scroll::abs(0.0))
                        .map(cosmic::action::app);

                let jump = match self.pending_jump_line.take() {
                    Some(line) => Task::done(cosmic::action::app(Message::JumpToLine(line))),
                    None => Task::none(),
                };

                if let State::Ready {
                    editor, preview, ..
                } = &mut self.state
                {
                    return utils::images::download_images(
                        &mut preview.markstate,
                        &mut preview.images_in_progress,
                        &editor.path,
                    )
                    .chain(reset_editor)
                    .chain(reset_preview)
                    .chain(jump);
                }

                Task::none()
            }
            Err(e) => self.handle_add_toast(PostillaToast::new(e)),
        }
    }

    pub fn handle_file_saved(
        &mut self,
        result: Option<Result<PathBuf, anywho::Error>>,
    ) -> Task<cosmic::Action<Message>> {
        if let Some(res) = result {
            match res {
                Ok(new_path) => {
                    let State::Ready { editor, .. } = &mut self.state else {
                        return Task::none();
                    };

                    editor.path = Some(new_path);
                    editor.is_dirty = false;
                    editor.history.mark_saved();

                    if std::mem::take(&mut self.rewatch_after_save) {
                        self.watch_generation += 1;
                    }

                    if let Some(action) = self.pending_after_save.take() {
                        self.silent_save = false;
                        return self.handle_discard_changes(action);
                    }

                    if std::mem::take(&mut self.silent_save) {
                        Task::none()
                    } else {
                        self.handle_add_toast(PostillaToast::new(crate::fl!("file-saved")))
                    }
                }
                Err(e) => {
                    self.silent_save = false;
                    self.pending_after_save = None;
                    self.handle_add_toast(PostillaToast::new(e))
                }
            }
        } else {
            // the save dialog was cancelled, stay on the current file
            self.silent_save = false;
            self.pending_after_save = None;
            let State::Ready { editor, .. } = &mut self.state else {
                return Task::none();
            };
            editor.ignore_next_external_change = false;

            Task::none()
        }
    }

    pub fn handle_discard_changes(
        &mut self,
        action: DiscardChangesAction,
    ) -> Task<cosmic::Action<Message>> {
        match action {
            DiscardChangesAction::CloseApp => {
                std::process::exit(0);
            }
            DiscardChangesAction::OpenFile(path) => {
                Task::perform(utils::files::load_file(path), |res| {
                    cosmic::action::app(Message::OpenFile(res))
                })
            }
            DiscardChangesAction::CloseFile(path) => {
                self.remove_nav_node(&path);
                self.handle_new_file()
            }
        }
    }

    /// Whether the "changed / deleted externally" dialog is already on screen
    fn external_change_dialog_shown(&self) -> bool {
        use crate::app::dialogs::DialogPage;
        self.dialog_pages.iter().any(|d| {
            matches!(
                d,
                DialogPage::ExternalFileModified(_) | DialogPage::ExternalFileRemoved(_)
            )
        })
    }

    pub fn handle_external_file_changed(&mut self, path: PathBuf) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &mut self.state else {
            return Task::none();
        };

        if editor.ignore_next_external_change {
            editor.ignore_next_external_change = false;
            return Task::none();
        }

        let currently_open = editor.path.as_ref() == Some(&path);
        if currently_open && !self.external_change_dialog_shown() {
            let State::Ready { editor, .. } = &mut self.state else {
                return Task::none();
            };
            // dirty so "keep my version" writes the editor text back, and no undo
            // step matches the file on disk any more
            editor.is_dirty = true;
            editor.history.saved_index = None;
            // the file may have been replaced (delete + rename), watch the new one
            self.watch_generation += 1;
            self.dialog_pages
                .push_back(crate::app::dialogs::DialogPage::ExternalFileModified(path));
        }

        Task::none()
    }

    pub fn handle_external_file_removed(&mut self, path: PathBuf) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &mut self.state else {
            return Task::none();
        };

        if editor.ignore_next_external_change {
            editor.ignore_next_external_change = false;
            return Task::none();
        }

        let currently_open = editor.path.as_ref() == Some(&path);
        if currently_open && !self.external_change_dialog_shown() {
            let State::Ready { editor, .. } = &mut self.state else {
                return Task::none();
            };
            // dirty so "keep my version" recreates the file from the editor text
            editor.is_dirty = true;
            editor.history.saved_index = None;
            self.rewatch_after_save = true;
            self.dialog_pages
                .push_back(crate::app::dialogs::DialogPage::ExternalFileRemoved(path));
        }

        Task::none()
    }
}

/// Returns the previous state of the panes if available, default if not
fn get_previous_pane_state(
    app_model: &AppModel,
) -> cosmic::widget::pane_grid::State<crate::app::PaneContent> {
    let State::Ready { panes, .. } = &app_model.state else {
        return create_default_panes();
    };

    panes.clone()
}

/// Returns the previous view mode if available, default if not
fn get_previous_view_mode(app_model: &AppModel) -> ViewMode {
    let State::Ready { view_mode, .. } = &app_model.state else {
        return app_model.config.last_view_mode;
    };

    *view_mode
}
