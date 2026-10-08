// SPDX-License-Identifier: GPL-3.0

use crate::config::ViewMode;
use crate::{app::Message, fl};
use cosmic::widget::menu::{self, ItemHeight, ItemWidth, KeyBind};
use cosmic::{Apply, Element};
use std::collections::HashMap;

/// Represents a Action that executes after clicking on the application Menu
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    /// Open the About [`ContextPage`] of the application
    About,
    /// Open the Settings [`ContextPage`] of the application
    Settings,
    /// Open a FileDialog to pick a new file to open
    OpenFile,
    /// Create a new empty file
    NewFile,
    /// Create a new vault file
    NewVaultFile,
    /// Create a new vault folder
    NewVaultFolder,
    /// Save the current file
    SaveFile,
    /// Export the current document to PDF (needs Gotenberg)
    ExportPdf,
    /// Export the current document to a standalone HTML file
    ExportHtml,
    /// Copy the current document as HTML to the clipboard
    CopyHtml,

    /// Undo
    Undo,
    /// Redo
    Redo,
    /// Cut the selection to the clipboard
    Cut,
    /// Copy the selection to the clipboard
    Copy,
    /// Paste from the clipboard (images are pasted as markdown images)
    Paste,
    /// Paste image from clipboard
    PasteImage,
    /// Select the whole document
    SelectAll,
    /// Search in the current document
    Search,
    /// Search across every note in the vault
    SearchVault,
    /// Insert the current date and time
    InsertDateTime,

    /// Formatting
    Bold,
    Italic,
    Strikethrough,
    Code,
    Link,
    Quote,
    Table,

    /// Show only the editor
    ViewEditor,
    /// Show editor and preview
    ViewSplit,
    /// Show only the preview
    ViewPreview,
    /// Toggle the preview for the current file
    TogglePreview,
    /// Toggle the distraction free (zen) mode
    ToggleZenMode,
    /// Show the document outline
    Outline,

    /// Close the current open popup dialog
    CloseCurrentDialog,
}

impl MenuAction {
    /// The [`ViewMode`] this action switches to, if any
    pub fn view_mode(self) -> Option<ViewMode> {
        match self {
            MenuAction::ViewEditor => Some(ViewMode::Editor),
            MenuAction::ViewSplit => Some(ViewMode::Split),
            MenuAction::ViewPreview => Some(ViewMode::Preview),
            _ => None,
        }
    }
}

impl menu::action::MenuAction for MenuAction {
    type Message = crate::app::Message;

    fn message(&self) -> Self::Message {
        Message::MenuAction(*self)
    }
}

pub fn menu_bar<'a>(
    key_binds: &HashMap<KeyBind, MenuAction>,
    view_mode: ViewMode,
    zen_mode: bool,
    gotenberg_configured: bool,
) -> Element<'a, Message> {
    let export_pdf = if gotenberg_configured {
        menu::Item::Button(fl!("export-pdf"), None, MenuAction::ExportPdf)
    } else {
        menu::Item::ButtonDisabled(fl!("export-pdf"), None, MenuAction::ExportPdf)
    };

    menu::bar(vec![
        menu::Tree::with_children(
            menu::root(fl!("file")).apply(Element::from),
            menu::items(
                key_binds,
                vec![
                    menu::Item::Button(fl!("new-vault-file"), None, MenuAction::NewVaultFile),
                    menu::Item::Button(fl!("new-folder"), None, MenuAction::NewVaultFolder),
                    menu::Item::Button(fl!("open-file"), None, MenuAction::OpenFile),
                    menu::Item::Button(fl!("save-file"), None, MenuAction::SaveFile),
                    menu::Item::Divider,
                    menu::Item::Button(fl!("new-file"), None, MenuAction::NewFile),
                    menu::Item::Divider,
                    menu::Item::Folder(
                        fl!("export"),
                        vec![
                            menu::Item::Button(fl!("export-html"), None, MenuAction::ExportHtml),
                            menu::Item::Button(fl!("copy-html"), None, MenuAction::CopyHtml),
                            export_pdf,
                        ],
                    ),
                ],
            ),
        ),
        menu::Tree::with_children(
            menu::root(fl!("edit")).apply(Element::from),
            menu::items(
                key_binds,
                vec![
                    menu::Item::Button(fl!("undo"), None, MenuAction::Undo),
                    menu::Item::Button(fl!("redo"), None, MenuAction::Redo),
                    menu::Item::Divider,
                    menu::Item::Button(fl!("cut"), None, MenuAction::Cut),
                    menu::Item::Button(fl!("copy"), None, MenuAction::Copy),
                    menu::Item::Button(fl!("paste"), None, MenuAction::Paste),
                    menu::Item::Button(fl!("paste-image"), None, MenuAction::PasteImage),
                    menu::Item::Button(fl!("select-all"), None, MenuAction::SelectAll),
                    menu::Item::Divider,
                    menu::Item::Button(fl!("search"), None, MenuAction::Search),
                    menu::Item::Button(fl!("search-vault"), None, MenuAction::SearchVault),
                    menu::Item::Divider,
                    menu::Item::Button(fl!("insert-datetime"), None, MenuAction::InsertDateTime),
                ],
            ),
        ),
        menu::Tree::with_children(
            menu::root(fl!("format")).apply(Element::from),
            menu::items(
                key_binds,
                vec![
                    menu::Item::Button(fl!("bold"), None, MenuAction::Bold),
                    menu::Item::Button(fl!("italic"), None, MenuAction::Italic),
                    menu::Item::Button(fl!("strikethrough"), None, MenuAction::Strikethrough),
                    menu::Item::Button(fl!("code"), None, MenuAction::Code),
                    menu::Item::Button(fl!("link"), None, MenuAction::Link),
                    menu::Item::Button(fl!("quote"), None, MenuAction::Quote),
                    menu::Item::Button(fl!("table"), None, MenuAction::Table),
                ],
            ),
        ),
        menu::Tree::with_children(
            menu::root(fl!("view")).apply(Element::from),
            menu::items(
                key_binds,
                vec![
                    menu::Item::CheckBox(
                        fl!("view-editor"),
                        None,
                        view_mode == ViewMode::Editor,
                        MenuAction::ViewEditor,
                    ),
                    menu::Item::CheckBox(
                        fl!("view-split"),
                        None,
                        view_mode == ViewMode::Split,
                        MenuAction::ViewSplit,
                    ),
                    menu::Item::CheckBox(
                        fl!("view-preview"),
                        None,
                        view_mode == ViewMode::Preview,
                        MenuAction::ViewPreview,
                    ),
                    menu::Item::Divider,
                    menu::Item::CheckBox(fl!("zen-mode"), None, zen_mode, MenuAction::ToggleZenMode),
                    menu::Item::Button(fl!("outline"), None, MenuAction::Outline),
                    menu::Item::Divider,
                    menu::Item::Button(fl!("settings"), None, MenuAction::Settings),
                    menu::Item::Button(fl!("about"), None, MenuAction::About),
                ],
            ),
        ),
    ])
    .item_height(ItemHeight::Dynamic(40))
    .item_width(ItemWidth::Uniform(270))
    .spacing(4.0)
    .into()
}

/// Items of the right click context menu of the text editor
pub fn editor_context_menu(
    key_binds: &HashMap<KeyBind, MenuAction>,
) -> Vec<menu::Tree<Message>> {
    menu::items(
        key_binds,
        vec![
            menu::Item::Button(fl!("cut"), None, MenuAction::Cut),
            menu::Item::Button(fl!("copy"), None, MenuAction::Copy),
            menu::Item::Button(fl!("paste"), None, MenuAction::Paste),
            menu::Item::Button(fl!("select-all"), None, MenuAction::SelectAll),
            menu::Item::Divider,
            menu::Item::Button(fl!("bold"), None, MenuAction::Bold),
            menu::Item::Button(fl!("italic"), None, MenuAction::Italic),
            menu::Item::Button(fl!("strikethrough"), None, MenuAction::Strikethrough),
            menu::Item::Button(fl!("code"), None, MenuAction::Code),
            menu::Item::Button(fl!("link"), None, MenuAction::Link),
            menu::Item::Divider,
            menu::Item::Button(fl!("insert-datetime"), None, MenuAction::InsertDateTime),
            menu::Item::Button(fl!("toggle-preview"), None, MenuAction::TogglePreview),
        ],
    )
}
