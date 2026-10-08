// SPDX-License-Identifier: GPL-3.0

use crate::app::core::utils::{self, vault_search::VaultSearchAction};
use crate::app::{AppModel, Message, State};
use cosmic::prelude::*;

impl AppModel {
    pub fn handle_vault_search(
        &mut self,
        action: VaultSearchAction,
    ) -> Task<cosmic::Action<Message>> {
        match action {
            VaultSearchAction::UpdateQuery(query) => {
                self.vault_search.query = query;
                self.run_vault_search()
            }
            VaultSearchAction::ToggleRegex => {
                self.vault_search.use_regex = !self.vault_search.use_regex;
                self.run_vault_search()
            }
            VaultSearchAction::Results(generation, results) => {
                if generation == self.vault_search.generation {
                    self.vault_search.searching = false;
                    self.vault_search.results = results;
                }
                Task::none()
            }
            VaultSearchAction::Failed(generation, error) => {
                if generation == self.vault_search.generation {
                    self.vault_search.searching = false;
                    self.vault_search.results.clear();
                    self.vault_search.error = Some(error);
                }
                Task::none()
            }
            VaultSearchAction::Open(path, line) => {
                let State::Ready { editor, .. } = &self.state else {
                    return Task::none();
                };

                self.pending_jump_line = Some(line);

                if editor.path.as_ref() == Some(&path) {
                    self.pending_jump_line = None;
                    return self.handle_jump_to_line(line);
                }

                if editor.is_dirty && editor.needs_confirmation() {
                    return self.handle_dialog_action(
                        crate::app::dialogs::DialogAction::OpenConfirmCloseFileDialog(
                            crate::app::DiscardChangesAction::OpenFile(path),
                        ),
                    );
                }

                Task::perform(utils::files::load_file(path), |res| {
                    cosmic::action::app(Message::OpenFile(res))
                })
            }
        }
    }

    /// Starts a new search with the current query, discarding results of older searches
    fn run_vault_search(&mut self) -> Task<cosmic::Action<Message>> {
        self.vault_search.generation = self.vault_search.generation.wrapping_add(1);
        self.vault_search.error = None;

        let query = self.vault_search.query.clone();
        if query.trim().is_empty() {
            self.vault_search.searching = false;
            self.vault_search.results.clear();
            return Task::none();
        }

        self.vault_search.searching = true;
        let generation = self.vault_search.generation;
        let root = self.config.vault_path();
        let use_regex = self.vault_search.use_regex;

        Task::perform(
            async move {
                let result = tokio::task::spawn_blocking(move || {
                    utils::vault_search::search_vault(&root, &query, use_regex)
                })
                .await;

                match result {
                    Ok(Ok(results)) => Ok(results),
                    Ok(Err(err)) => Err(err),
                    Err(err) => Err(err.to_string()),
                }
            },
            move |result| match result {
                Ok(results) => cosmic::action::app(Message::VaultSearch(
                    VaultSearchAction::Results(generation, results),
                )),
                Err(err) => cosmic::action::app(Message::VaultSearch(
                    VaultSearchAction::Failed(generation, err),
                )),
            },
        )
    }
}
