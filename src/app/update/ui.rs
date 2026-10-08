// SPDX-License-Identifier: GPL-3.0

use crate::app::context_page::ContextPage;
use crate::app::core::utils::PostillaToast;
use crate::app::{AppModel, Message};
use crate::fl;
use cosmic::iced::core::keyboard::{Key, Modifiers};
use cosmic::widget::ToastId;
use cosmic::{iced, prelude::*, surface};

impl AppModel {
    pub fn handle_close_toast(&mut self, id: ToastId) -> Task<cosmic::Action<Message>> {
        self.toasts.remove(id);
        Task::none()
    }

    pub fn handle_add_toast(&mut self, toast: PostillaToast) -> Task<cosmic::Action<Message>> {
        self.toasts.push(toast.into()).map(cosmic::action::app)
    }

    pub fn handle_launch_url(&mut self, url: String) -> Task<cosmic::Action<Message>> {
        match open::that_detached(&url) {
            Ok(()) => Task::none(),
            Err(err) => {
                eprintln!("failed to open {url:?}: {err}");
                Task::none()
            }
        }
    }

    /// Writes text to the system clipboard. The system clipboard (arboard) is
    /// tried first, the toolkit clipboard is the fallback.
    pub fn clipboard_write(&mut self, content: String) -> Task<cosmic::Action<Message>> {
        if let Some(clipboard) = &mut self.clipboard {
            match clipboard.set_text(content.clone()) {
                Ok(()) => return Task::none(),
                Err(err) => eprintln!("system clipboard write failed: {err}"),
            }
        }

        iced::clipboard::write(content)
    }

    /// Reads text from the system clipboard synchronously, if possible
    pub fn clipboard_read(&mut self) -> Option<String> {
        let clipboard = self.clipboard.as_mut()?;
        match clipboard.get_text() {
            Ok(text) => Some(text),
            Err(err) => {
                eprintln!("system clipboard read failed: {err}");
                None
            }
        }
    }

    pub fn handle_copy_to_clipboard(&mut self, content: String) -> Task<cosmic::Action<Message>> {
        let write = self.clipboard_write(content);
        let toast = self.handle_add_toast(PostillaToast::new(fl!("copied-to-clipboard")));
        write.chain(toast)
    }

    pub fn handle_toggle_context_page(
        &mut self,
        page: ContextPage,
    ) -> Task<cosmic::Action<Message>> {
        page.toggle_context_page(self)
    }

    pub fn handle_surface(&mut self, a: surface::Action<Message>) -> Task<cosmic::Action<Message>> {
        cosmic::task::message(cosmic::Action::Surface(a))
    }

    pub fn handle_key(&mut self, modifiers: Modifiers, key: Key) -> Task<cosmic::Action<Message>> {
        for (key_bind, action) in self.key_binds.iter() {
            if key_bind.matches(modifiers, &key, None) {
                return self.handle_menu_action(*action);
            }
        }
        Task::none()
    }

    pub fn handle_modifiers(&mut self, modifiers: Modifiers) -> Task<cosmic::Action<Message>> {
        self.modifiers = modifiers;
        Task::none()
    }

    /// Toggles the distraction free mode
    pub fn handle_toggle_zen_mode(&mut self) -> Task<cosmic::Action<Message>> {
        self.zen_mode = !self.zen_mode;

        if self.zen_mode {
            self.zen_restore_nav = self.core.nav_bar_active();
            self.core.nav_bar_set_toggled(false);
            self.core.window.show_headerbar = false;
            self.core.window.show_context = false;
        } else {
            self.core.nav_bar_set_toggled(self.zen_restore_nav);
            self.core.window.show_headerbar = true;
        }

        Task::none()
    }
}
