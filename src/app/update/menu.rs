// SPDX-License-Identifier: GPL-3.0

use crate::app::app_menu::MenuAction;
use crate::app::context_page::ContextPage;
use crate::app::core::utils::{self, SelectionAction};
use crate::app::{AppModel, ClipboardAction, Message};
use crate::app::{State, dialogs};
use crate::config::ViewMode;
use cosmic::prelude::*;

impl AppModel {
    pub fn handle_menu_action(&mut self, action: MenuAction) -> Task<cosmic::Action<Message>> {
        let State::Ready { view_mode, .. } = &self.state else {
            return Task::none();
        };
        let view_mode = *view_mode;

        if let Some(mode) = action.view_mode() {
            return self.handle_set_view_mode(mode);
        }

        match action {
            MenuAction::About => self.handle_toggle_context_page(ContextPage::About),
            MenuAction::Settings => self.handle_toggle_context_page(ContextPage::Settings),
            MenuAction::Outline => self.handle_toggle_context_page(ContextPage::Outline),
            MenuAction::SearchVault => {
                let task = self.handle_toggle_context_page(ContextPage::VaultSearch);
                if self.core.window.show_context {
                    task.chain(cosmic::widget::text_input::focus(
                        crate::app::vault_search_input_id(),
                    ))
                } else {
                    task
                }
            }
            MenuAction::OpenFile => Task::perform(
                async move {
                    match utils::files::open_markdown_file_picker().await {
                        Some(path) => Some(utils::files::load_file(path.into()).await),
                        None => None,
                    }
                },
                |res| match res {
                    Some(result) => cosmic::action::app(Message::OpenFile(result)),
                    None => cosmic::action::none(),
                },
            ),
            MenuAction::NewFile => self.handle_new_file(),
            MenuAction::NewVaultFile => {
                self.handle_dialog_action(dialogs::DialogAction::OpenNewVaultFileDialog)
            }
            MenuAction::NewVaultFolder => {
                self.handle_dialog_action(dialogs::DialogAction::OpenNewVaultFolderDialog)
            }
            MenuAction::SaveFile => self.handle_save_file(),
            MenuAction::ExportPdf => self.handle_export_pdf(),
            MenuAction::ExportHtml => self.handle_export_html(),
            MenuAction::CopyHtml => self.handle_copy_html(),
            MenuAction::TogglePreview => {
                let next = match view_mode {
                    ViewMode::Split => ViewMode::Editor,
                    ViewMode::Editor | ViewMode::Preview => ViewMode::Split,
                };
                self.handle_set_view_mode(next)
            }
            MenuAction::ViewEditor | MenuAction::ViewSplit | MenuAction::ViewPreview => {
                Task::none() // handled above
            }
            MenuAction::ToggleZenMode => self.handle_toggle_zen_mode(),
            MenuAction::ToggleSidebar => {
                if self.zen_mode {
                    self.zen_restore_nav = !self.zen_restore_nav;
                } else {
                    self.core.nav_bar_toggle();
                }
                Task::none()
            }
            MenuAction::ToggleHelpBar => {
                let next = if self.config.show_helper_header_bar.is_shown() {
                    crate::config::ShowState::Hide
                } else {
                    crate::config::ShowState::Show
                };
                self.handle_config_input(crate::config::ConfigInput::HelperHeaderBarShowState(
                    next,
                ))
            }
            MenuAction::ToggleStatusBar => {
                let next = if self.config.show_status_bar.is_shown() {
                    crate::config::ShowState::Hide
                } else {
                    crate::config::ShowState::Show
                };
                self.handle_config_input(crate::config::ConfigInput::StatusBarShowState(next))
            }
            MenuAction::Undo => self.handle_undo(),
            MenuAction::Redo => self.handle_redo(),
            MenuAction::Cut => self.handle_clipboard(ClipboardAction::Cut),
            MenuAction::Copy => self.handle_clipboard(ClipboardAction::Copy),
            MenuAction::Paste => self.handle_clipboard(ClipboardAction::Paste),
            MenuAction::SelectAll => self.handle_clipboard(ClipboardAction::SelectAll),
            MenuAction::PasteImage => self.handle_paste_image(),
            MenuAction::Search => self.handle_search(utils::search::SearchAction::ToggleSearch),
            MenuAction::InsertDateTime => self.handle_apply_formatting(SelectionAction::DateTime),
            MenuAction::Bold => self.handle_apply_formatting(SelectionAction::Bold),
            MenuAction::Italic => self.handle_apply_formatting(SelectionAction::Italic),
            MenuAction::Strikethrough => {
                self.handle_apply_formatting(SelectionAction::Strikethrough)
            }
            MenuAction::Code => self.handle_apply_formatting(SelectionAction::Code),
            MenuAction::Link => self.handle_apply_formatting(SelectionAction::Hyperlink),
            MenuAction::Quote => self.handle_apply_formatting(SelectionAction::Blockquote),
            MenuAction::Table => self.handle_apply_formatting(SelectionAction::Table),
            MenuAction::CloseCurrentDialog => {
                if self.dialog_pages.is_empty() {
                    if self.zen_mode {
                        return self.handle_toggle_zen_mode();
                    }
                    Task::none()
                } else {
                    self.handle_dialog_action(dialogs::DialogAction::DialogCancel)
                }
            }
        }
    }
}
