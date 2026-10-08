// SPDX-License-Identifier: GPL-3.0

use crate::app::core::utils::{self, PostillaToast};
use crate::app::{AppModel, DiscardChangesAction, Message, State, dialogs};
use crate::config::ShowState;
use crate::fl;
use cosmic::prelude::*;
use std::path::Path;
use std::process;

impl AppModel {
    /// Opens the given path in the default file explorer (or default app if target is not a directory)
    pub fn handle_open_in_file_explorer(&self, path: &Path) -> Task<cosmic::Action<Message>> {
        match open::that(path) {
            Ok(()) => Task::none(),
            Err(err) => {
                eprintln!("failed to open {path:?}: {err}");
                Task::none()
            }
        }
    }

    /// Exports the current document to PDF, rendered offline with Typst
    pub fn handle_export_pdf(&mut self) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &self.state else {
            return Task::none();
        };

        let content = editor.content.text();
        if content.trim().is_empty() {
            return Task::none();
        }

        let file_path = editor.path.clone();

        Task::perform(
            async move {
                match utils::files::open_pdf_file_saver().await {
                    Some(destination) => {
                        Some(utils::typst_export::export_pdf(file_path, content, destination).await)
                    }
                    // Error selecting where to save the file
                    None => None,
                }
            },
            |res| match res {
                Some(Ok(())) => cosmic::action::app(Message::AddToast(PostillaToast::new(fl!(
                    "pdf-exported"
                )))),
                Some(Err(e)) => cosmic::action::app(Message::AddToast(PostillaToast::new(e))),
                None => cosmic::action::none(),
            },
        )
    }

    /// Show / Quit from the tray icon
    pub fn handle_tray(&mut self, event: crate::tray::TrayEvent) -> Task<cosmic::Action<Message>> {
        let show = match self.core.main_window_id() {
            Some(id) => Task::batch([
                cosmic::iced::window::minimize(id, false),
                cosmic::iced::window::gain_focus(id),
            ]),
            None => Task::none(),
        };

        match event {
            crate::tray::TrayEvent::Show => show,
            crate::tray::TrayEvent::Quit => {
                show.chain(Task::done(cosmic::action::app(Message::AppCloseRequested)))
            }
        }
    }

    /// Exports the current document as a standalone HTML file
    pub fn handle_export_html(&mut self) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &self.state else {
            return Task::none();
        };

        let content = editor.content.text();
        if content.trim().is_empty() {
            return Task::none();
        }

        let file_path = editor.path.clone();
        let suggested_name = file_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| format!("{}.html", s.to_string_lossy()));

        Task::perform(
            async move {
                match utils::files::open_html_file_saver(suggested_name).await {
                    Some(destination) => {
                        Some(utils::html::export_html(file_path, content, destination).await)
                    }
                    None => None,
                }
            },
            |res| match res {
                Some(Ok(())) => cosmic::action::app(Message::AddToast(PostillaToast::new(fl!(
                    "html-exported"
                )))),
                Some(Err(e)) => cosmic::action::app(Message::AddToast(PostillaToast::new(e))),
                None => cosmic::action::none(),
            },
        )
    }

    /// Copies the current document as HTML to the clipboard
    pub fn handle_copy_html(&mut self) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &self.state else {
            return Task::none();
        };

        let content = editor.content.text();
        let file_path = editor.path.clone();

        Task::perform(
            async move { utils::html::markdown_to_html_document(file_path, &content).await },
            |html| cosmic::action::app(Message::CopyToClipboard(html)),
        )
    }

    pub fn handle_app_close_requested(&mut self) -> Task<cosmic::Action<Message>> {
        let State::Ready {
            editor, view_mode, ..
        } = &self.state
        else {
            return Task::none();
        };

        if let Some(handler) = &self.config_handler {
            let current_nav_state = if self.zen_mode {
                self.zen_restore_nav
            } else {
                self.core.nav_bar_active()
            };
            let current_nav_state = if current_nav_state {
                ShowState::Show
            } else {
                ShowState::Hide
            };

            if let Err(err) = self.config.set_last_view_mode(handler, *view_mode) {
                eprintln!("{err}");
            }

            if let Err(err) = self
                .config
                .set_last_navbar_showstate(handler, current_nav_state)
            {
                eprintln!("{err}");
            }

            if let Err(err) = self.config.set_last_open_file(handler, editor.path.clone()) {
                eprintln!("{err}");
            }
        }

        if editor.is_dirty {
            // if it's a vault path with any modification or if it's a new file with any content
            if editor.needs_confirmation() {
                self.handle_dialog_action(dialogs::DialogAction::OpenConfirmCloseFileDialog(
                    DiscardChangesAction::CloseApp,
                ))
            } else {
                process::exit(0);
            }
        } else {
            process::exit(0);
        }
    }
}
