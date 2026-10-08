// SPDX-License-Identifier: GPL-3.0

use crate::app::core::utils::Image;
use crate::app::{AppModel, Message, State, editor_scrollable_id, preview_scrollable_id};
use crate::config::{BoolState, ViewMode};
use cosmic::iced::widget::scrollable::scroll_to;
use cosmic::iced::widget::{pane_grid, scrollable};
use cosmic::prelude::*;
use cosmic::widget::{self, segmented_button};
use frostmark::UpdateMsg;

impl AppModel {
    pub fn handle_update_mark_state(
        &mut self,
        message: UpdateMsg,
    ) -> Task<cosmic::Action<Message>> {
        if let State::Ready { preview, .. } = &mut self.state {
            preview.markstate.update(message)
        }
        Task::none()
    }

    pub fn handle_image_downloaded(
        &mut self,
        res: Result<Image, anywho::Error>,
    ) -> Task<cosmic::Action<Message>> {
        let State::Ready { preview, .. } = &mut self.state else {
            return Task::none();
        };

        match res {
            Ok(image) => {
                if image.is_svg {
                    preview.insert_svg(image.url, image.bytes);
                } else {
                    preview.insert_image(image.url, image.bytes);
                }
            }
            Err(err) => {
                eprintln!("Couldn't download image: {err}");
            }
        }

        Task::none()
    }

    /// Changes the layout of the main view
    pub fn handle_set_view_mode(&mut self, mode: ViewMode) -> Task<cosmic::Action<Message>> {
        let State::Ready { view_mode, .. } = &mut self.state else {
            return Task::none();
        };

        *view_mode = mode;

        // keep the header bar switcher in sync
        let entity = self
            .view_mode_model
            .iter()
            .find(|e| self.view_mode_model.data::<ViewMode>(*e) == Some(&mode));
        if let Some(entity) = entity {
            self.view_mode_model.activate(entity);
        }

        // remember the layout
        if let Some(handler) = &self.config_handler
            && let Err(err) = self.config.set_last_view_mode(handler, mode)
        {
            eprintln!("{err}");
        }

        // the preview viewport is stale after being hidden, drop it so the scroll
        // sync doesn't jump when it comes back
        if let State::Ready { editor, .. } = &mut self.state
            && !mode.shows_preview()
        {
            editor.scroll.last_preview_viewport = None;
        }

        Task::none()
    }

    pub fn handle_view_mode_selected(
        &mut self,
        entity: segmented_button::Entity,
    ) -> Task<cosmic::Action<Message>> {
        match self.view_mode_model.data::<ViewMode>(entity).copied() {
            Some(mode) => self.handle_set_view_mode(mode),
            None => Task::none(),
        }
    }

    pub fn handle_pane_resized(
        &mut self,
        event: pane_grid::ResizeEvent,
    ) -> Task<cosmic::Action<Message>> {
        let State::Ready { panes, .. } = &mut self.state else {
            return Task::none();
        };

        panes.resize(event.split, event.ratio);
        Task::none()
    }

    pub fn handle_pane_dragged(
        &mut self,
        event: pane_grid::DragEvent,
    ) -> Task<cosmic::Action<Message>> {
        let State::Ready { panes, .. } = &mut self.state else {
            return Task::none();
        };

        if let pane_grid::DragEvent::Dropped { pane, target } = event {
            panes.drop(pane, target);
        }

        Task::none()
    }

    pub fn handle_scroll_changed(
        &mut self,
        source_id: widget::Id,
        viewport: scrollable::Viewport,
    ) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &mut self.state else {
            return Task::none();
        };

        let is_editor = source_id == editor_scrollable_id();
        let is_preview = source_id == preview_scrollable_id();

        if is_editor {
            editor.scroll.last_editor_viewport = Some(viewport);

            // programmatic scroll we fired, consume it and skip sync
            if editor.scroll.pending_editor_scrolls > 0 {
                editor.scroll.pending_editor_scrolls -= 1;
                return Task::none();
            }

            // user initiated editor scroll, check if we need to sync
            if self.config.scrollbar_sync != BoolState::Yes {
                return Task::none();
            }

            // we have a viewport?
            let Some(preview_vp) = editor.scroll.last_preview_viewport else {
                return Task::none();
            };

            let target_y = crate::app::utils::scroll::proportional_y(viewport, preview_vp);
            editor.scroll.pending_preview_scrolls += 1;
            return scroll_to(
                preview_scrollable_id(),
                crate::app::utils::scroll::abs(target_y),
            )
            .map(cosmic::action::app);
        }

        if is_preview {
            let prev_height = editor.scroll.last_preview_content_height;
            let new_height = viewport.content_bounds().height;

            editor.scroll.last_preview_viewport = Some(viewport);
            editor.scroll.last_preview_content_height = new_height;

            // programmatic scroll we fired, consume it and skip sync
            if editor.scroll.pending_preview_scrolls > 0 {
                editor.scroll.pending_preview_scrolls -= 1;
                return Task::none();
            }

            // Content height changed, this is a reflow from images/fonts loading,
            // not a user scroll. Ignore it to avoid snapping the editor.
            if (prev_height - new_height).abs() > 1.0 {
                return Task::none();
            }

            // user initiated preview scroll, check if we need to sync
            if self.config.scrollbar_sync != BoolState::Yes {
                return Task::none();
            }

            // we have a viewport?
            let Some(editor_vp) = editor.scroll.last_editor_viewport else {
                return Task::none();
            };

            let target_y = crate::app::utils::scroll::proportional_y(viewport, editor_vp);
            editor.scroll.pending_editor_scrolls += 1;
            return scroll_to(
                editor_scrollable_id(),
                crate::app::utils::scroll::abs(target_y),
            )
            .map(cosmic::action::app);
        }

        Task::none()
    }
}
