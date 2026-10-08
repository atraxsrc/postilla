// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;

use cosmic::iced::keyboard::Key;
use cosmic::iced::keyboard::key::Named;
use cosmic::widget::menu::key_bind::KeyBind;
use cosmic::widget::menu::key_bind::Modifier;

use crate::app::app_menu::MenuAction;

/// COSMIC handled keybinds
pub fn key_binds() -> HashMap<KeyBind, MenuAction> {
    let mut key_binds = HashMap::new();

    macro_rules! bind {
        ([$($modifier:ident),* $(,)?], $key:expr, $action:ident) => {{
            key_binds.insert(
                KeyBind {
                    modifiers: vec![$(Modifier::$modifier),*],
                    key: $key,
                },
                MenuAction::$action,
            );
        }};
    }

    // File
    bind!([Ctrl], Key::Character("t".into()), NewVaultFile);
    bind!([Ctrl], Key::Character("n".into()), NewVaultFolder);
    bind!([Ctrl, Shift], Key::Character("t".into()), NewFile);
    bind!([Ctrl], Key::Character("s".into()), SaveFile);
    bind!([Ctrl], Key::Character("o".into()), OpenFile);
    bind!([Ctrl, Shift], Key::Character("e".into()), ExportHtml);

    // Edit
    bind!([Ctrl], Key::Character("z".into()), Undo);
    bind!([Ctrl, Shift], Key::Character("z".into()), Redo);
    bind!([Ctrl], Key::Character("x".into()), Cut);
    bind!([Ctrl], Key::Character("c".into()), Copy);
    bind!([Ctrl], Key::Character("v".into()), Paste);
    bind!([Ctrl], Key::Character("a".into()), SelectAll);
    bind!([Ctrl], Key::Character("p".into()), PasteImage);
    bind!([Ctrl], Key::Character("f".into()), Search);
    bind!([Ctrl, Shift], Key::Character("f".into()), SearchVault);
    bind!([Ctrl, Shift], Key::Character("d".into()), InsertDateTime);

    // Format
    bind!([Ctrl], Key::Character("b".into()), Bold);
    bind!([Ctrl], Key::Character("i".into()), Italic);
    bind!([Ctrl, Shift], Key::Character("x".into()), Strikethrough);
    bind!([Ctrl, Shift], Key::Character("k".into()), Code);
    bind!([Ctrl], Key::Character("k".into()), Link);
    bind!([Ctrl, Shift], Key::Character("q".into()), Quote);

    // View
    bind!([Ctrl], Key::Character("1".into()), ViewEditor);
    bind!([Ctrl], Key::Character("2".into()), ViewSplit);
    bind!([Ctrl], Key::Character("3".into()), ViewPreview);
    bind!([Ctrl], Key::Character("h".into()), TogglePreview);
    bind!([], Key::Named(Named::F11), ToggleZenMode);
    bind!([Ctrl, Shift], Key::Character("o".into()), Outline);

    bind!([Ctrl], Key::Character(",".into()), Settings);
    bind!([Ctrl, Shift], Key::Character("i".into()), About);

    bind!([], Key::Named(Named::Escape), CloseCurrentDialog);

    key_binds
}
