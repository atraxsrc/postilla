// SPDX-License-Identifier: GPL-3.0

use std::{fmt::Display, path::PathBuf, sync::LazyLock};

use cosmic::{
    Application,
    cosmic_config::{self, Config, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
    theme,
};
use serde::{Deserialize, Serialize};

use crate::fl;

pub const CONFIG_VERSION: u64 = 1;

#[derive(Debug, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct PostillaConfig {
    pub app_theme: AppTheme,
    pub vault_path: String,
    pub show_helper_header_bar: ShowState,
    pub show_status_bar: ShowState,
    pub last_navbar_showstate: ShowState,
    /// Layout used the last time the app was closed
    pub last_view_mode: ViewMode,
    pub open_last_file: BoolState,
    pub last_open_file: Option<PathBuf>,
    pub scrollbar_sync: BoolState,
    pub text_size: i32,
    pub light_highlighter_theme: SyntaxThemeId,
    pub dark_highlighter_theme: SyntaxThemeId,
    /// Paint the editor and the preview code blocks with the colors of the
    /// syntax highlighting theme (background/foreground) instead of the
    /// application theme colors.
    pub highlighter_theme_colors: BoolState,
    pub selected_font_family: Option<String>,
    /// Frosted glass (blur behind the window) behaviour
    pub frosted_glass: FrostedGlass,
    /// Window background opacity in percent (100 = opaque)
    pub window_opacity: u8,
    /// Custom window background color as hex (`#rrggbb`), empty = theme default
    pub window_background: String,
    /// Seconds of inactivity before an open file is saved automatically (0 = off)
    pub autosave_delay: u32,
}

impl Default for PostillaConfig {
    fn default() -> Self {
        let vault_path = dirs::data_dir()
            .unwrap()
            .join(crate::app::AppModel::APP_ID)
            .join("vault");

        if !vault_path.exists() {
            std::fs::create_dir_all(&vault_path).expect("Failed to create vault directory");
        }

        Self {
            app_theme: AppTheme::default(),
            vault_path: vault_path.to_string_lossy().to_string(),
            show_helper_header_bar: ShowState::default(),
            show_status_bar: ShowState::default(),
            last_navbar_showstate: ShowState::default(),
            last_view_mode: ViewMode::default(),
            open_last_file: BoolState::default(),
            last_open_file: None,
            scrollbar_sync: BoolState::default(),
            text_size: 16,
            light_highlighter_theme: SyntaxThemeId::new(crate::syntax_theme::DEFAULT_LIGHT),
            dark_highlighter_theme: SyntaxThemeId::new(crate::syntax_theme::DEFAULT_DARK),
            highlighter_theme_colors: BoolState::No,
            selected_font_family: None,
            frosted_glass: FrostedGlass::default(),
            window_opacity: 100,
            window_background: String::new(),
            autosave_delay: 0,
        }
    }
}

impl PostillaConfig {
    pub fn config_handler() -> Option<Config> {
        Config::new(crate::app::AppModel::APP_ID, CONFIG_VERSION).ok()
    }

    pub fn config() -> Self {
        match Self::config_handler() {
            Some(config_handler) => {
                PostillaConfig::get_entry(&config_handler).unwrap_or_else(|(error, config)| {
                    eprintln!("Error whilst loading config: {error:#?}");
                    config
                })
            }
            None => PostillaConfig::default(),
        }
    }

    /// Returns the current vault path
    pub fn vault_path(&self) -> PathBuf {
        PathBuf::from(&self.vault_path)
    }

    /// Window opacity clamped to a usable range, as a factor
    pub fn window_opacity_factor(&self) -> f32 {
        f32::from(self.window_opacity.clamp(MIN_WINDOW_OPACITY, 100)) / 100.0
    }

    /// Whether the window should be drawn with a custom (translucent/frosted/recolored) theme
    pub fn needs_custom_theme(&self) -> bool {
        self.frosted_glass != FrostedGlass::FollowSystem
            || self.window_opacity < 100
            || !self.window_background.trim().is_empty()
            || self.highlighter_theme_colors.is_yes()
    }
}

/// Lowest window opacity the settings allow
pub const MIN_WINDOW_OPACITY: u8 = 30;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum AppTheme {
    Dark,
    Light,
    #[default]
    System,
}

impl AppTheme {
    pub fn theme(&self) -> theme::Theme {
        match self {
            Self::Dark => theme::Theme::dark(),
            Self::Light => theme::Theme::light(),
            Self::System => theme::system_preference(),
        }
    }
}

/// Layout of the main view
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ViewMode {
    /// Only the text editor
    Editor,
    /// Editor and preview side by side
    #[default]
    Split,
    /// Only the rendered preview
    Preview,
}

impl ViewMode {
    pub const ALL: &'static [Self] = &[Self::Editor, Self::Split, Self::Preview];

    pub fn label(self) -> String {
        match self {
            ViewMode::Editor => fl!("view-editor"),
            ViewMode::Split => fl!("view-split"),
            ViewMode::Preview => fl!("view-preview"),
        }
    }

    pub fn icon_name(self) -> &'static str {
        match self {
            ViewMode::Editor => "edit-symbolic",
            ViewMode::Split => "view-split-symbolic",
            ViewMode::Preview => "show-symbolic",
        }
    }

    pub fn shows_editor(self) -> bool {
        !matches!(self, ViewMode::Preview)
    }

    pub fn shows_preview(self) -> bool {
        !matches!(self, ViewMode::Editor)
    }
}

/// Frosted glass (compositor blur behind the window) behaviour
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum FrostedGlass {
    /// Follow the COSMIC desktop setting ("Frosted windows")
    #[default]
    FollowSystem,
    /// Always request blur, even if the desktop theme does not enable it
    Enabled,
    /// Never request blur
    Disabled,
}

impl FrostedGlass {
    pub const ALL: &'static [Self] = &[Self::FollowSystem, Self::Enabled, Self::Disabled];

    pub fn all_labels() -> &'static [String] {
        static LABELS: LazyLock<Vec<String>> = LazyLock::new(|| {
            vec![
                fl!("frosted-follow-system"),
                fl!("frosted-enabled"),
                fl!("frosted-disabled"),
            ]
        });
        &LABELS
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or_default()
    }

    pub fn to_index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }
}

/// Autosave delays offered in the settings (seconds, 0 = off)
pub const AUTOSAVE_DELAYS: &[u32] = &[0, 1, 3, 5, 10, 30, 60];

pub fn autosave_labels() -> &'static [String] {
    static LABELS: LazyLock<Vec<String>> = LazyLock::new(|| {
        AUTOSAVE_DELAYS
            .iter()
            .map(|secs| {
                if *secs == 0 {
                    fl!("autosave-off")
                } else {
                    fl!("autosave-seconds", seconds = secs)
                }
            })
            .collect()
    });
    &LABELS
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ShowState {
    #[default]
    Show,
    Hide,
}

impl Display for ShowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShowState::Show => write!(f, "{}", fl!("show")),
            ShowState::Hide => write!(f, "{}", fl!("hide")),
        }
    }
}

impl ShowState {
    pub fn all_labels() -> &'static [String] {
        static LABELS: LazyLock<Vec<String>> = LazyLock::new(|| vec![fl!("show"), fl!("hide")]);
        &LABELS
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => ShowState::Show,
            1 => ShowState::Hide,
            _ => ShowState::default(),
        }
    }

    pub fn to_index(self) -> usize {
        match self {
            ShowState::Show => 0,
            ShowState::Hide => 1,
        }
    }

    pub fn is_shown(self) -> bool {
        matches!(self, ShowState::Show)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum BoolState {
    #[default]
    Yes,
    No,
}

impl Display for BoolState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoolState::Yes => write!(f, "{}", fl!("yes")),
            BoolState::No => write!(f, "{}", fl!("no")),
        }
    }
}

impl BoolState {
    pub fn all_labels() -> &'static [String] {
        static LABELS: LazyLock<Vec<String>> = LazyLock::new(|| vec![fl!("yes"), fl!("no")]);
        &LABELS
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => BoolState::Yes,
            1 => BoolState::No,
            _ => BoolState::default(),
        }
    }

    pub fn to_index(self) -> usize {
        match self {
            BoolState::Yes => 0,
            BoolState::No => 1,
        }
    }

    pub fn is_yes(self) -> bool {
        matches!(self, BoolState::Yes)
    }
}

/// Identifier of a syntax highlighting theme, see [`crate::syntax_theme`].
///
/// Older versions of Postilla stored the index of an `iced::highlighter::Theme`;
/// those values are still accepted when deserializing.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SyntaxThemeId(pub String);

impl SyntaxThemeId {
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for SyntaxThemeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SyntaxThemeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IdVisitor;

        impl serde::de::Visitor<'_> for IdVisitor {
            type Value = SyntaxThemeId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a theme id string or a legacy theme index")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(SyntaxThemeId(v.to_string()))
            }

            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(SyntaxThemeId(v))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                crate::syntax_theme::legacy_index_to_id(v)
                    .map(SyntaxThemeId)
                    .ok_or_else(|| E::custom(format!("invalid theme index: {v}")))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                if v < 0 {
                    return Err(E::custom(format!("invalid theme index: {v}")));
                }
                self.visit_u64(v as u64)
            }
        }

        deserializer.deserialize_any(IdVisitor)
    }
}

/// Represents the different inputs that can happen in the config [`ContextPage`]
#[derive(Debug, Clone)]
pub enum ConfigInput {
    /// Automatic theme detection changed
    SystemThemeModeChange,
    /// Update the application theme
    UpdateTheme(usize),
    /// Update the help bar show state
    HelperHeaderBarShowState(ShowState),
    /// Update the status bar show state
    StatusBarShowState(ShowState),
    /// Update if the user wants to open last opened file or not
    OpenLastFile(BoolState),
    /// Update if the user wants the editor and preview scrollbars to be in sync
    ScrollbarSync(BoolState),
    /// Update the editor and preview text size
    UpdateTextSize(u16),
    /// Update the highlighter theme for light app themes (index in the theme registry)
    UpdateLightHighlighterTheme(usize),
    /// Update the highlighter theme for dark app themes (index in the theme registry)
    UpdateDarkHighlighterTheme(usize),
    /// Use the highlighter theme colors for the editor background/foreground
    HighlighterThemeColors(BoolState),
    /// Pick a theme file (RON / tmTheme) to import into the themes folder
    ImportSyntaxTheme,
    /// Callback after importing a theme file
    SyntaxThemeImported(Result<String, String>),
    /// Open the themes folder in the file manager
    OpenThemesFolder,
    /// Re-read the themes folder
    ReloadSyntaxThemes,
    /// Update the selected font for the preview and editor tabs
    UpdateFont(usize),
    /// Reset to the default font
    ResetFont,
    /// Update the frosted glass behaviour
    FrostedGlass(usize),
    /// Update the window opacity (percent)
    WindowOpacity(u8),
    /// Update the custom window background color (hex, empty = theme default)
    WindowBackground(String),
    /// Update the autosave delay (index in [`AUTOSAVE_DELAYS`])
    Autosave(usize),
}
