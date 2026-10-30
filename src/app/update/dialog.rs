// SPDX-License-Identifier: GPL-3.0

use crate::app::dialogs;
use crate::app::{AppModel, Message, NavMenuAction, State};
use cosmic::prelude::*;
use cosmic::widget::segmented_button;
use slotmap::Key as SlotmapKey;

impl AppModel {
    pub fn handle_dialog_action(
        &mut self,
        action: dialogs::DialogAction,
    ) -> Task<cosmic::Action<Message>> {
        let State::Ready { .. } = &mut self.state else {
            return Task::none();
        };

        action.execute(&mut self.dialog_pages, &self.dialog_state)
    }

    /// Name of a vault node as shown in the sidebar
    fn node_display_name(&self, entity: segmented_button::Entity) -> String {
        self.nav_model
            .data::<crate::app::core::project::ProjectNode>(entity)
            .map(|node| node.name().to_string())
            .unwrap_or_default()
    }

    pub fn handle_nav_bar_context(
        &mut self,
        entity: segmented_button::Entity,
    ) -> Task<cosmic::Action<Message>> {
        self.nav_bar_context_id = entity;
        Task::none()
    }

    pub fn handle_nav_menu_action(
        &mut self,
        action: NavMenuAction,
    ) -> Task<cosmic::Action<Message>> {
        self.nav_bar_context_id = segmented_button::Entity::null();

        match action {
            NavMenuAction::DeleteNode(entity) => {
                let name = self.node_display_name(entity);
                self.handle_dialog_action(dialogs::DialogAction::OpenDeleteNodeDialog(
                    entity, name,
                ))
            }
            NavMenuAction::RenameNode(entity) => {
                // pre-fill the current name, without the extension for files
                let name = match self
                    .nav_model
                    .data::<crate::app::core::project::ProjectNode>(entity)
                {
                    Some(crate::app::core::project::ProjectNode::File { path, .. }) => path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    Some(node) => node.name().to_string(),
                    None => String::new(),
                };
                self.handle_dialog_action(dialogs::DialogAction::OpenRenameNodeDialog(
                    entity, name,
                ))
            }
            NavMenuAction::MoveNode(entity) => {
                let vault_path = self.config.vault_path();
                self.dialog_state.available_folders = self.collect_all_folders(&vault_path, entity);

                self.handle_dialog_action(dialogs::DialogAction::OpenMoveNodeDialog(entity))
            }
            NavMenuAction::OpenNodeFileManager(entity) => {
                if let Some(entity) = self
                    .nav_model
                    .data::<crate::app::core::project::ProjectNode>(entity)
                {
                    let path = match entity {
                        crate::app::core::project::ProjectNode::Folder { path, .. } => path,
                        crate::app::core::project::ProjectNode::File { path, .. } => {
                            // a file should always have a parent
                            path.parent().unwrap()
                        }
                    };

                    self.handle_open_in_file_explorer(path)
                } else {
                    Task::none()
                }
            }
            NavMenuAction::OpenFolderCreationDialog(entity) => {
                if let Some(entity) = self
                    .nav_model
                    .data::<crate::app::core::project::ProjectNode>(entity)
                {
                    let path = match entity {
                        crate::app::core::project::ProjectNode::Folder { path, .. } => path,
                        crate::app::core::project::ProjectNode::File { path, .. } => {
                            // a file should always have a parent
                            path.parent().unwrap()
                        }
                    };

                    // we update the selected nav path becasue the context menu click does not update it so
                    // if the user hasn't left clicked first it has not updated
                    self.selected_nav_path = Some(path.to_path_buf());
                    self.handle_dialog_action(dialogs::DialogAction::OpenNewVaultFolderDialog)
                } else {
                    Task::none()
                }
            }
            NavMenuAction::OpenVaultFileCreationDialog(entity) => {
                if let Some(entity) = self
                    .nav_model
                    .data::<crate::app::core::project::ProjectNode>(entity)
                {
                    let path = match entity {
                        crate::app::core::project::ProjectNode::Folder { path, .. } => path,
                        crate::app::core::project::ProjectNode::File { path, .. } => {
                            // a file should always have a parent
                            path.parent().unwrap()
                        }
                    };

                    // we update the selected nav path becasue the context menu click does not update it so
                    // if the user hasn't left clicked first it has not updated
                    self.selected_nav_path = Some(path.to_path_buf());
                    self.handle_dialog_action(dialogs::DialogAction::OpenNewVaultFileDialog)
                } else {
                    Task::none()
                }
            }
        }
    }
}
