// SPDX-License-Identifier: GPL-3.0

use std::sync::Arc;

use crate::app::core::utils::{self, PostillaToast};
use crate::app::{AppModel, Message};
use crate::config::{
    AUTOSAVE_DELAYS, AppTheme, PostillaConfig, ConfigInput, FrostedGlass, MIN_WINDOW_OPACITY,
    SyntaxThemeId,
};
use crate::fl;
use crate::syntax_theme::ThemeRegistry;
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::cosmic_theme::ThemeBuilder;
use cosmic::iced::Font;
use cosmic::prelude::*;
use enumflags2::BitFlags;

impl AppModel {
    /// Applies a config change via the handler, falling back to an in-memory update if the handler fails or is missing.
    fn apply_config<F>(&mut self, updater: F) -> Task<cosmic::Action<Message>>
    where
        F: FnOnce(&mut PostillaConfig, Option<&cosmic::cosmic_config::Config>) -> Result<(), String>,
    {
        if let Some(handler) = &self.config_handler {
            if let Err(err) = updater(&mut self.config, Some(handler)) {
                eprintln!("{err}");
            }
        } else {
            let _ = updater(&mut self.config, None);
        }
        Task::none()
    }

    /// Builds the application theme from the configuration (app theme, frosted
    /// glass behaviour and window opacity) and applies it.
    pub fn apply_theme(&mut self) -> Task<cosmic::Action<Message>> {
        // Let libcosmic request compositor blur for our surfaces unless the user disabled it
        let auto_blur = if self.config.frosted_glass == FrostedGlass::Disabled {
            BitFlags::empty()
        } else {
            cosmic::core::Auto::Window | cosmic::core::Auto::Popup | cosmic::core::Auto::System
        };
        self.core.set_auto_blur(auto_blur);

        cosmic::command::set_theme(self.build_theme())
    }

    /// The theme to use for the window, see [`Self::apply_theme`]
    fn build_theme(&self) -> cosmic::Theme {
        let base = self.config.app_theme.theme();

        if !self.config.needs_custom_theme() {
            return base;
        }

        let is_dark = base.cosmic().is_dark;

        // Start from the desktop's theme builder so accent, palette... are kept
        let builder_config = if is_dark {
            ThemeBuilder::dark_config()
        } else {
            ThemeBuilder::light_config()
        };
        let mut builder = match builder_config {
            Ok(config) => ThemeBuilder::get_entry(&config).unwrap_or_else(|(_, builder)| builder),
            Err(_) if is_dark => ThemeBuilder::dark(),
            Err(_) => ThemeBuilder::light(),
        };

        // Background color: explicit hex setting first, then the highlighter theme
        // background when the user wants the theme colors everywhere
        let custom_rgb = crate::syntax_theme::parse_color(&self.config.window_background)
            .ok()
            .map(|c| (c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0))
            .or_else(|| {
                self.config
                    .highlighter_theme_colors
                    .is_yes()
                    .then(|| widgets::highlighter::theme_background(self.active_syntax_theme()))
                    .flatten()
                    .map(|c| (c.r, c.g, c.b))
            });

        let opacity = self.config.window_opacity_factor();
        if custom_rgb.is_some() || self.config.window_opacity < 100 {
            let mut bg = base.cosmic().bg_color();
            if let Some((r, g, b)) = custom_rgb {
                bg.red = r;
                bg.green = g;
                bg.blue = b;
            }
            // used when the window is drawn without compositor blur
            bg.alpha = opacity;
            builder = builder.bg_color(bg);
        }

        if self.config.window_opacity < 100 {
            // With blur active libcosmic paints the "frosted" variants whose alpha
            // comes from this table (indexed by the blur strength), so the slider
            // has to drive the table rather than the base color alpha.
            builder.alpha_map = cosmic::cosmic_theme::AlphaMap {
                extremely_low: opacity,
                extremely_low_2: opacity,
                very_low: opacity,
                very_low_2: opacity,
                low: opacity,
                low_2: opacity,
                medium: opacity,
                medium_2: opacity,
                high: opacity,
                high_2: opacity,
                very_high: opacity,
                very_high_2: opacity,
                extremely_high: opacity,
                extremely_high_2: opacity,
            };
        }

        match self.config.frosted_glass {
            FrostedGlass::FollowSystem => {}
            FrostedGlass::Enabled => builder.frosted_windows = true,
            FrostedGlass::Disabled => builder.frosted_windows = false,
        }

        cosmic::Theme::custom(Arc::new(builder.build()))
    }

    /// Stores the given syntax theme id for light or dark app themes
    fn set_highlighter_theme(&mut self, index: usize, dark: bool) -> Task<cosmic::Action<Message>> {
        let Some(theme) = self.syntax_themes.themes().get(index) else {
            return Task::none();
        };
        let id = SyntaxThemeId::new(&theme.id);

        self.apply_config(move |config, handler| {
            if dark {
                if let Some(h) = handler {
                    config
                        .set_dark_highlighter_theme(h, id)
                        .map_err(|e| e.to_string())?;
                } else {
                    config.dark_highlighter_theme = id;
                }
            } else if let Some(h) = handler {
                config
                    .set_light_highlighter_theme(h, id)
                    .map_err(|e| e.to_string())?;
            } else {
                config.light_highlighter_theme = id;
            }
            Ok(())
        })
    }

    pub fn handle_config_input(&mut self, input: ConfigInput) -> Task<cosmic::Action<Message>> {
        match input {
            ConfigInput::SystemThemeModeChange => self.apply_theme(),
            ConfigInput::UpdateTheme(index) => {
                let app_theme = match index {
                    1 => AppTheme::Dark,
                    2 => AppTheme::Light,
                    _ => AppTheme::System,
                };

                if let Some(handler) = &self.config_handler {
                    if let Err(err) = self.config.set_app_theme(handler, app_theme) {
                        eprintln!("{err}");
                        self.config.app_theme = app_theme;
                    }
                } else {
                    self.config.app_theme = app_theme;
                }

                self.apply_theme()
            }
            ConfigInput::FrostedGlass(index) => {
                let mode = FrostedGlass::from_index(index);
                let _ = self.apply_config(move |config, handler| {
                    if let Some(h) = handler {
                        config.set_frosted_glass(h, mode).map_err(|e| e.to_string())?;
                    } else {
                        config.frosted_glass = mode;
                    }
                    Ok(())
                });
                self.apply_theme()
            }
            ConfigInput::WindowOpacity(value) => {
                let value = value.clamp(MIN_WINDOW_OPACITY, 100);
                let _ = self.apply_config(move |config, handler| {
                    if let Some(h) = handler {
                        config.set_window_opacity(h, value).map_err(|e| e.to_string())?;
                    } else {
                        config.window_opacity = value;
                    }
                    Ok(())
                });
                self.apply_theme()
            }
            ConfigInput::WindowBackground(value) => {
                let value = value.trim().to_string();
                let _ = self.apply_config(move |config, handler| {
                    if let Some(h) = handler {
                        config
                            .set_window_background(h, value)
                            .map_err(|e| e.to_string())?;
                    } else {
                        config.window_background = value;
                    }
                    Ok(())
                });
                self.apply_theme()
            }
            ConfigInput::HighlighterThemeColors(state) => {
                let _ = self.apply_config(|config, handler| {
                    if let Some(h) = handler {
                        config
                            .set_highlighter_theme_colors(h, state)
                            .map_err(|e| e.to_string())?;
                    } else {
                        config.highlighter_theme_colors = state;
                    }
                    Ok(())
                });
                // the window background may follow the highlighter theme now
                self.apply_theme()
            }
            ConfigInput::UpdateLightHighlighterTheme(index) => {
                let task = self.set_highlighter_theme(index, false);
                task.chain(self.apply_theme())
            }
            ConfigInput::UpdateDarkHighlighterTheme(index) => {
                let task = self.set_highlighter_theme(index, true);
                task.chain(self.apply_theme())
            }
            ConfigInput::Autosave(index) => {
                let delay = AUTOSAVE_DELAYS.get(index).copied().unwrap_or(0);
                let _ = self.apply_config(move |config, handler| {
                    if let Some(h) = handler {
                        config.set_autosave_delay(h, delay).map_err(|e| e.to_string())?;
                    } else {
                        config.autosave_delay = delay;
                    }
                    Ok(())
                });
                self.schedule_autosave()
            }
            ConfigInput::HelperHeaderBarShowState(show_state) => {
                self.apply_config(|config, handler| {
                    if let Some(h) = handler {
                        config
                            .set_show_helper_header_bar(h, show_state)
                            .map_err(|e| e.to_string())?;
                    } else {
                        config.show_helper_header_bar = show_state;
                    }
                    Ok(())
                })
            }
            ConfigInput::StatusBarShowState(show_state) => self.apply_config(|config, handler| {
                if let Some(h) = handler {
                    config
                        .set_show_status_bar(h, show_state)
                        .map_err(|e| e.to_string())?;
                } else {
                    config.show_status_bar = show_state;
                }
                Ok(())
            }),
            ConfigInput::OpenLastFile(state) => self.apply_config(|config, handler| {
                if let Some(h) = handler {
                    config
                        .set_open_last_file(h, state)
                        .map_err(|e| e.to_string())?;
                } else {
                    config.open_last_file = state;
                }
                Ok(())
            }),
            ConfigInput::ScrollbarSync(state) => self.apply_config(|config, handler| {
                if let Some(h) = handler {
                    config
                        .set_scrollbar_sync(h, state)
                        .map_err(|e| e.to_string())?;
                } else {
                    config.scrollbar_sync = state;
                }
                Ok(())
            }),
            ConfigInput::UpdateTextSize(new_size) => {
                let size = new_size as i32;
                self.apply_config(|config, handler| {
                    if let Some(h) = handler {
                        config.set_text_size(h, size).map_err(|e| e.to_string())?;
                    } else {
                        config.text_size = size;
                    }
                    Ok(())
                })
            }
            ConfigInput::ImportSyntaxTheme => Task::perform(
                async move { utils::files::open_theme_file_picker().await },
                |path| match path {
                    Some(path) => cosmic::action::app(Message::ConfigInput(
                        ConfigInput::SyntaxThemeImported(
                            // the registry lives on the model, import happens there
                            Ok(path.to_string_lossy().into_owned()),
                        ),
                    )),
                    None => cosmic::action::none(),
                },
            ),
            ConfigInput::SyntaxThemeImported(result) => {
                let path = match result {
                    Ok(path) => std::path::PathBuf::from(path),
                    Err(err) => {
                        return self.handle_add_toast(PostillaToast::new(fl!(
                            "theme-import-failed",
                            error = err
                        )));
                    }
                };

                match self.syntax_themes.import(&path) {
                    Ok(id) => {
                        self.syntax_theme_names = self.syntax_themes.names();
                        let name = self
                            .syntax_themes
                            .get(&id)
                            .map(|t| t.name.clone())
                            .unwrap_or_else(|| id.clone());
                        let is_dark = self.syntax_themes.get(&id).is_some_and(|t| t.is_dark);

                        // select the imported theme for the matching mode right away
                        let theme_id = SyntaxThemeId::new(&id);
                        let _ = self.apply_config(move |config, handler| {
                            if is_dark {
                                if let Some(h) = handler {
                                    config
                                        .set_dark_highlighter_theme(h, theme_id)
                                        .map_err(|e| e.to_string())?;
                                } else {
                                    config.dark_highlighter_theme = theme_id;
                                }
                            } else if let Some(h) = handler {
                                config
                                    .set_light_highlighter_theme(h, theme_id)
                                    .map_err(|e| e.to_string())?;
                            } else {
                                config.light_highlighter_theme = theme_id;
                            }
                            Ok(())
                        });

                        let toast = self.handle_add_toast(PostillaToast::new(fl!(
                            "theme-imported",
                            name = name
                        )));
                        // the window background may follow the imported theme
                        toast.chain(self.apply_theme())
                    }
                    Err(err) => self.handle_add_toast(PostillaToast::new(fl!(
                        "theme-import-failed",
                        error = err.to_string()
                    ))),
                }
            }
            ConfigInput::OpenThemesFolder => {
                self.handle_open_in_file_explorer(&ThemeRegistry::user_dir())
            }
            ConfigInput::ReloadSyntaxThemes => {
                self.syntax_themes.reload_user_themes();
                self.syntax_theme_names = self.syntax_themes.names();
                // the window background may follow a theme that changed on disk
                self.apply_theme()
            }
            ConfigInput::UpdateFont(index) => {
                if let Some(name) = self.system_fonts.get(index).cloned() {
                    // TODO: Is this the only way we can do this?
                    let static_name: &'static str = Box::leak(name.clone().into_boxed_str());

                    self.postilla_font = Font {
                        family: cosmic::iced::font::Family::Name(static_name),
                        weight: cosmic::iced::font::Weight::Normal,
                        stretch: cosmic::iced::font::Stretch::Normal,
                        style: cosmic::iced::font::Style::Normal,
                    };

                    self.apply_config(move |config, handler| {
                        if let Some(h) = handler {
                            config
                                .set_selected_font_family(h, Some(name))
                                .map_err(|e| e.to_string())?;
                        } else {
                            config.selected_font_family = Some(name);
                        }
                        Ok(())
                    })
                } else {
                    Task::none()
                }
            }
            ConfigInput::ResetFont => {
                self.postilla_font = Font::DEFAULT;

                self.apply_config(|config, handler| {
                    if let Some(h) = handler {
                        config
                            .set_selected_font_family(h, None)
                            .map_err(|e| e.to_string())?;
                    } else {
                        config.selected_font_family = None;
                    }
                    Ok(())
                })
            }
        }
    }
}
