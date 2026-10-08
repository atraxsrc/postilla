// SPDX-License-Identifier: GPL-3.0

//! Syntax highlighting theme registry.
//!
//! Cedilla ships the syntect default themes and loads user themes from the
//! themes folder. Three formats are supported and detected automatically:
//!
//! - **Cedilla RON themes**: a flat list of named colors (`keyword`, `string`,
//!   `heading`...), see `resources/themes/example-cedilla-theme.ron`.
//! - **COSMIC Terminal color schemes**: the RON files COSMIC Terminal
//!   imports/exports. The 16 ANSI colors are mapped onto syntax scopes.
//! - **TextMate / Sublime `.tmTheme`** files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anywho::anywho;
use serde::Deserialize;
use widgets::highlighter::syntect::highlighting::{
    Color, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSet, ThemeSettings,
};
use widgets::highlighter::{self as hl, ThemeRef};

/// Prefix of theme ids that refer to the syntect default themes.
const BUILTIN_PREFIX: &str = "builtin:";
/// Prefix of theme ids that refer to files in the themes folder.
const USER_PREFIX: &str = "user:";

/// The default theme used with light application themes.
pub const DEFAULT_LIGHT: &str = "builtin:InspiredGitHub";
/// The default theme used with dark application themes.
pub const DEFAULT_DARK: &str = "builtin:base16-ocean.dark";

/// Built-in themes: (id key, display name). The order of the first five
/// entries matches `iced::highlighter::Theme::ALL` so old configs (which
/// stored an index into that list) keep pointing at the same theme.
const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("Solarized (dark)", "Solarized Dark"),
    ("base16-mocha.dark", "Mocha"),
    ("base16-ocean.dark", "Ocean"),
    ("base16-eighties.dark", "Eighties"),
    ("InspiredGitHub", "Inspired GitHub"),
    ("base16-ocean.light", "Ocean Light"),
    ("Solarized (light)", "Solarized Light"),
];

/// Converts a legacy config value (an index into `iced::highlighter::Theme::ALL`)
/// into a theme id.
pub fn legacy_index_to_id(index: u64) -> Option<String> {
    BUILTIN_THEMES
        .get(index as usize)
        .map(|(key, _)| format!("{BUILTIN_PREFIX}{key}"))
}

/// A loaded syntax highlighting theme.
#[derive(Debug, Clone)]
pub struct SyntaxTheme {
    /// Stable identifier stored in the configuration.
    pub id: String,
    /// Human readable name.
    pub name: String,
    /// Whether the theme has a dark background.
    pub is_dark: bool,
    /// The file the theme was loaded from, `None` for built-in themes.
    pub path: Option<PathBuf>,
    /// The syntect theme.
    pub theme: ThemeRef,
}

/// All themes known to the application.
#[derive(Debug, Default)]
pub struct ThemeRegistry {
    themes: Vec<SyntaxTheme>,
}

impl ThemeRegistry {
    /// Loads the built-in themes and every theme in the user themes folder.
    pub fn load() -> Self {
        let mut registry = Self::default();

        for (key, name) in BUILTIN_THEMES {
            if let Some(theme) = hl::default_theme(key) {
                registry.themes.push(SyntaxTheme {
                    id: format!("{BUILTIN_PREFIX}{key}"),
                    name: (*name).to_string(),
                    is_dark: hl::theme_is_dark(theme),
                    path: None,
                    theme,
                });
            }
        }

        registry.reload_user_themes();
        registry
    }

    /// The folder user themes are loaded from (created and seeded with the
    /// example themes if missing).
    pub fn user_dir() -> PathBuf {
        let dir = dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(crate::app::APP_ID)
            .join("themes");

        if !dir.exists() && std::fs::create_dir_all(&dir).is_ok() {
            // Give users something to start from
            let examples: &[(&str, &str)] = &[
                (
                    "README.md",
                    include_str!("../resources/themes/README.md"),
                ),
                (
                    "example-cedilla-theme.ron",
                    include_str!("../resources/themes/example-cedilla-theme.ron"),
                ),
                (
                    "example-cosmic-term.ron",
                    include_str!("../resources/themes/example-cosmic-term.ron"),
                ),
            ];
            for (name, content) in examples {
                let _ = std::fs::write(dir.join(name), content);
            }
        }

        dir
    }

    /// Re-reads the user themes folder.
    pub fn reload_user_themes(&mut self) {
        self.themes.retain(|t| t.path.is_none());

        let dir = Self::user_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };

        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| is_theme_file(p))
            .collect();
        paths.sort();

        for path in paths {
            match load_theme_file(&path) {
                Ok((name, theme)) => {
                    let theme = hl::leak_theme(theme);
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("theme")
                        .to_string();
                    self.themes.push(SyntaxTheme {
                        id: format!("{USER_PREFIX}{stem}"),
                        name,
                        is_dark: hl::theme_is_dark(theme),
                        path: Some(path),
                        theme,
                    });
                }
                Err(err) => eprintln!("Ignoring theme {}: {err}", path.display()),
            }
        }
    }

    /// Validates a theme file and copies it into the user themes folder.
    /// Returns the id of the imported theme.
    pub fn import(&mut self, source: &Path) -> Result<String, anywho::Error> {
        let (name, _) = load_theme_file(source)?;

        let file_name = source
            .file_name()
            .ok_or_else(|| anywho!("Invalid file name"))?;
        let destination = Self::user_dir().join(file_name);

        std::fs::copy(source, &destination).map_err(|e| anywho!("{e}"))?;
        self.reload_user_themes();

        let stem = destination
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("theme");

        eprintln!("Imported syntax theme '{name}' to {}", destination.display());
        Ok(format!("{USER_PREFIX}{stem}"))
    }

    pub fn themes(&self) -> &[SyntaxTheme] {
        &self.themes
    }

    pub fn names(&self) -> Vec<String> {
        self.themes.iter().map(|t| t.name.clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<&SyntaxTheme> {
        self.themes.iter().find(|t| t.id == id)
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.themes.iter().position(|t| t.id == id)
    }

    /// Resolves a theme id, falling back to the default for the given mode.
    pub fn resolve(&self, id: &str, dark: bool) -> ThemeRef {
        self.get(id)
            .or_else(|| self.get(if dark { DEFAULT_DARK } else { DEFAULT_LIGHT }))
            .or_else(|| self.themes.first())
            .map(|t| t.theme)
            .unwrap_or_else(|| {
                hl::default_theme("InspiredGitHub").expect("syntect ships InspiredGitHub")
            })
    }
}

fn is_theme_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| {
                ext.eq_ignore_ascii_case("ron")
                    || ext.eq_ignore_ascii_case("tmtheme")
                    || ext.eq_ignore_ascii_case("ron.txt")
            })
}

/// Loads a theme from disk, detecting the format from its content.
pub fn load_theme_file(path: &Path) -> Result<(String, Theme), anywho::Error> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    if ext == "tmtheme" {
        let theme = ThemeSet::get_theme(path).map_err(|e| anywho!("{e}"))?;
        let name = theme
            .name
            .clone()
            .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Theme".into());
        return Ok((name, theme));
    }

    let text = std::fs::read_to_string(path).map_err(|e| anywho!("{e}"))?;
    let fallback_name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Theme".into());

    parse_ron_theme(&text, &fallback_name)
}

/// Parses a RON theme (Cedilla or COSMIC Terminal format).
pub fn parse_ron_theme(text: &str, fallback_name: &str) -> Result<(String, Theme), anywho::Error> {
    let raw: RonTheme = ron::from_str(text).map_err(|e| anywho!("Invalid RON theme: {e}"))?;

    let name = raw
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| fallback_name.to_string());

    let theme = if raw.is_terminal_scheme() {
        raw.into_terminal_theme(&name)?
    } else {
        raw.into_cedilla_theme(&name)?
    };

    Ok((name, theme))
}

/// The union of both RON formats; every field is optional so either format
/// deserializes.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RonTheme {
    name: Option<String>,
    dark: Option<bool>,

    // Shared editor colors
    background: Option<String>,
    foreground: Option<String>,
    selection: Option<String>,
    caret: Option<String>,

    // COSMIC Terminal color scheme fields
    cursor: Option<String>,
    bright_foreground: Option<String>,
    dim_foreground: Option<String>,
    normal: Option<Ansi>,
    bright: Option<Ansi>,
    dim: Option<Ansi>,

    // Cedilla theme fields
    comment: Option<String>,
    keyword: Option<String>,
    string: Option<String>,
    number: Option<String>,
    constant: Option<String>,
    function: Option<String>,
    #[serde(rename = "type")]
    type_: Option<String>,
    variable: Option<String>,
    operator: Option<String>,
    punctuation: Option<String>,
    tag: Option<String>,
    attribute: Option<String>,
    inserted: Option<String>,
    deleted: Option<String>,
    heading: Option<String>,
    strong: Option<String>,
    emphasis: Option<String>,
    link: Option<String>,
    quote: Option<String>,
    list: Option<String>,
    code: Option<String>,
    scopes: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
struct Ansi {
    black: Option<String>,
    red: Option<String>,
    green: Option<String>,
    yellow: Option<String>,
    blue: Option<String>,
    magenta: Option<String>,
    cyan: Option<String>,
    white: Option<String>,
}

impl Ansi {
    fn is_empty(&self) -> bool {
        self.black.is_none()
            && self.red.is_none()
            && self.green.is_none()
            && self.yellow.is_none()
            && self.blue.is_none()
            && self.magenta.is_none()
            && self.cyan.is_none()
            && self.white.is_none()
    }
}

/// Scope selectors for each Cedilla color slot.
const SLOT_SCOPES: &[(&str, &str, FontStyle)] = &[
    ("comment", "comment, punctuation.definition.comment", FontStyle::ITALIC),
    (
        "keyword",
        "keyword, keyword.control, storage, storage.type, storage.modifier, keyword.other",
        FontStyle::empty(),
    ),
    (
        "string",
        "string, punctuation.definition.string, string.quoted",
        FontStyle::empty(),
    ),
    ("number", "constant.numeric", FontStyle::empty()),
    (
        "constant",
        "constant, constant.language, constant.character, constant.other, support.constant, variable.other.constant",
        FontStyle::empty(),
    ),
    (
        "function",
        "entity.name.function, support.function, meta.function-call, variable.function, entity.name.function.macro",
        FontStyle::empty(),
    ),
    (
        "type",
        "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, entity.name.trait, support.type, support.class, entity.other.inherited-class, storage.type.class",
        FontStyle::empty(),
    ),
    (
        "variable",
        "variable, variable.parameter, variable.other, entity.name.variable, variable.language",
        FontStyle::empty(),
    ),
    ("operator", "keyword.operator", FontStyle::empty()),
    (
        "punctuation",
        "punctuation, punctuation.separator, punctuation.terminator, punctuation.section, meta.brace",
        FontStyle::empty(),
    ),
    (
        "tag",
        "entity.name.tag, punctuation.definition.tag, meta.tag",
        FontStyle::empty(),
    ),
    (
        "attribute",
        "entity.other.attribute-name, meta.attribute",
        FontStyle::empty(),
    ),
    ("inserted", "markup.inserted", FontStyle::empty()),
    ("deleted", "markup.deleted, invalid, invalid.illegal", FontStyle::empty()),
    (
        "heading",
        "markup.heading, entity.name.section, punctuation.definition.heading",
        FontStyle::BOLD,
    ),
    ("strong", "markup.bold, punctuation.definition.bold", FontStyle::BOLD),
    (
        "emphasis",
        "markup.italic, punctuation.definition.italic",
        FontStyle::ITALIC,
    ),
    (
        "link",
        "markup.underline.link, string.other.link, meta.link, markup.underline, constant.other.reference.link",
        FontStyle::UNDERLINE,
    ),
    (
        "quote",
        "markup.quote, punctuation.definition.blockquote",
        FontStyle::ITALIC,
    ),
    (
        "list",
        "markup.list, punctuation.definition.list_item, markup.list.numbered.bullet",
        FontStyle::empty(),
    ),
    (
        "code",
        "markup.raw, markup.raw.inline, markup.raw.block, punctuation.definition.raw",
        FontStyle::empty(),
    ),
];

impl RonTheme {
    fn is_terminal_scheme(&self) -> bool {
        self.normal.as_ref().is_some_and(|a| !a.is_empty())
            || self.bright.as_ref().is_some_and(|a| !a.is_empty())
    }

    fn slot(&self, slot: &str) -> Option<&String> {
        match slot {
            "comment" => self.comment.as_ref(),
            "keyword" => self.keyword.as_ref(),
            "string" => self.string.as_ref(),
            "number" => self.number.as_ref(),
            "constant" => self.constant.as_ref(),
            "function" => self.function.as_ref(),
            "type" => self.type_.as_ref(),
            "variable" => self.variable.as_ref(),
            "operator" => self.operator.as_ref(),
            "punctuation" => self.punctuation.as_ref(),
            "tag" => self.tag.as_ref(),
            "attribute" => self.attribute.as_ref(),
            "inserted" => self.inserted.as_ref(),
            "deleted" => self.deleted.as_ref(),
            "heading" => self.heading.as_ref(),
            "strong" => self.strong.as_ref(),
            "emphasis" => self.emphasis.as_ref(),
            "link" => self.link.as_ref(),
            "quote" => self.quote.as_ref(),
            "list" => self.list.as_ref(),
            "code" => self.code.as_ref(),
            _ => None,
        }
    }

    fn into_cedilla_theme(self, name: &str) -> Result<Theme, anywho::Error> {
        let mut builder = ThemeBuilder::new(name);
        builder.background = parse_opt(self.background.as_deref())?;
        builder.foreground = parse_opt(self.foreground.as_deref())?;
        builder.selection = parse_opt(self.selection.as_deref())?;
        builder.caret = parse_opt(self.caret.as_deref().or(self.cursor.as_deref()))?;

        for (slot, selectors, style) in SLOT_SCOPES {
            if let Some(color) = self.slot(slot) {
                builder.push(selectors, parse_color(color)?, *style);
            }
        }

        for (selector, color) in &self.scopes {
            builder.push(selector, parse_color(color)?, FontStyle::empty());
        }

        Ok(builder.build())
    }

    fn into_terminal_theme(self, name: &str) -> Result<Theme, anywho::Error> {
        let normal = self.normal.clone().unwrap_or_default();
        let bright = self.bright.clone().unwrap_or_default();

        let pick = |a: &Option<String>, b: &Option<String>| -> Result<Option<Color>, anywho::Error> {
            parse_opt(a.as_deref().or(b.as_deref()))
        };

        let black = pick(&normal.black, &bright.black)?;
        let red = pick(&normal.red, &bright.red)?;
        let green = pick(&normal.green, &bright.green)?;
        let yellow = pick(&normal.yellow, &bright.yellow)?;
        let blue = pick(&normal.blue, &bright.blue)?;
        let magenta = pick(&normal.magenta, &bright.magenta)?;
        let cyan = pick(&normal.cyan, &bright.cyan)?;
        let white = pick(&normal.white, &bright.white)?;
        let bright_black = pick(&bright.black, &normal.black)?;
        let bright_yellow = pick(&bright.yellow, &normal.yellow)?;
        let bright_blue = pick(&bright.blue, &normal.blue)?;

        let mut builder = ThemeBuilder::new(name);
        builder.background = parse_opt(self.background.as_deref())?;
        builder.foreground = parse_opt(self.foreground.as_deref())?;
        builder.caret = parse_opt(self.cursor.as_deref().or(self.caret.as_deref()))?;
        builder.selection = match parse_opt(self.selection.as_deref())? {
            Some(c) => Some(c),
            None => bright_black.map(|mut c| {
                c.a = 0x88;
                c
            }),
        };

        let comment = parse_opt(self.dim_foreground.as_deref())?.or(bright_black).or(black);

        let mapping: &[(&str, Option<Color>)] = &[
            ("comment", comment),
            ("keyword", magenta),
            ("string", green),
            ("number", bright_yellow),
            ("constant", bright_yellow),
            ("function", blue),
            ("type", yellow),
            ("variable", red),
            ("operator", cyan),
            ("punctuation", white),
            ("tag", red),
            ("attribute", yellow),
            ("inserted", green),
            ("deleted", red),
            ("heading", bright_blue),
            ("strong", yellow),
            ("emphasis", magenta),
            ("link", cyan),
            ("quote", comment),
            ("list", red),
            ("code", green),
        ];

        for (slot, color) in mapping {
            if let Some(color) = color
                && let Some((_, selectors, style)) = SLOT_SCOPES.iter().find(|(s, _, _)| s == slot)
            {
                builder.push(selectors, *color, *style);
            }
        }

        Ok(builder.build())
    }
}

struct ThemeBuilder {
    name: String,
    background: Option<Color>,
    foreground: Option<Color>,
    selection: Option<Color>,
    caret: Option<Color>,
    scopes: Vec<ThemeItem>,
}

impl ThemeBuilder {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            background: None,
            foreground: None,
            selection: None,
            caret: None,
            scopes: Vec::new(),
        }
    }

    fn push(&mut self, selectors: &str, color: Color, font_style: FontStyle) {
        match ScopeSelectors::from_str(selectors) {
            Ok(scope) => self.scopes.push(ThemeItem {
                scope,
                style: StyleModifier {
                    foreground: Some(color),
                    background: None,
                    font_style: if font_style.is_empty() {
                        None
                    } else {
                        Some(font_style)
                    },
                },
            }),
            Err(err) => eprintln!("Ignoring invalid scope selector '{selectors}': {err:?}"),
        }
    }

    fn build(self) -> Theme {
        Theme {
            name: Some(self.name),
            author: None,
            settings: ThemeSettings {
                foreground: self.foreground,
                background: self.background,
                caret: self.caret,
                selection: self.selection,
                ..ThemeSettings::default()
            },
            scopes: self.scopes,
        }
    }
}

fn parse_opt(value: Option<&str>) -> Result<Option<Color>, anywho::Error> {
    value.map(parse_color).transpose()
}

/// Parses `#rgb`, `#rrggbb` or `#rrggbbaa` (the `#` is optional).
pub fn parse_color(value: &str) -> Result<Color, anywho::Error> {
    let hex = value.trim().trim_start_matches('#');
    let digits: Vec<u8> = hex
        .chars()
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| anywho!("Invalid color '{value}'"))?;

    let (r, g, b, a) = match digits.len() {
        3 => (
            digits[0] * 17,
            digits[1] * 17,
            digits[2] * 17,
            255,
        ),
        6 => (
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
            255,
        ),
        8 => (
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
            digits[6] * 16 + digits[7],
        ),
        _ => return Err(anywho!("Invalid color '{value}'")),
    };

    Ok(Color { r, g, b, a })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colors() {
        let c = parse_color("#1e1e2e").unwrap();
        assert_eq!((c.r, c.g, c.b, c.a), (0x1e, 0x1e, 0x2e, 255));
        let c = parse_color("abc").unwrap();
        assert_eq!((c.r, c.g, c.b), (0xaa, 0xbb, 0xcc));
        let c = parse_color("#11223344").unwrap();
        assert_eq!(c.a, 0x44);
        assert!(parse_color("nope").is_err());
    }

    #[test]
    fn parses_cosmic_term_scheme() {
        let text = include_str!("../resources/themes/example-cosmic-term.ron");
        let (name, theme) = parse_ron_theme(text, "fallback").unwrap();
        assert_eq!(name, "Example COSMIC Term Scheme");
        assert!(theme.settings.background.is_some());
        assert!(!theme.scopes.is_empty());
        assert!(hl::theme_is_dark(&theme));
    }

    #[test]
    fn parses_cedilla_theme() {
        let text = include_str!("../resources/themes/example-cedilla-theme.ron");
        let (name, theme) = parse_ron_theme(text, "fallback").unwrap();
        assert_eq!(name, "Example Cedilla Theme");
        // 21 slots + 1 raw scope override
        assert_eq!(theme.scopes.len(), 22);
    }

    #[test]
    fn legacy_indices_map_to_iced_order() {
        assert_eq!(legacy_index_to_id(4).as_deref(), Some(DEFAULT_LIGHT));
        assert_eq!(legacy_index_to_id(2).as_deref(), Some(DEFAULT_DARK));
        assert!(legacy_index_to_id(99).is_none());
    }
}
