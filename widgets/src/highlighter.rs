// SPDX-License-Identifier: GPL-3.0

//! A syntax highlighter for the Cedilla text editor and preview code blocks.
//!
//! This is a themeable variant of the `iced_highlighter` crate: instead of a
//! fixed set of built-in themes it highlights with any [`syntect`] theme,
//! which lets Cedilla load user supplied themes (tmTheme files, COSMIC
//! Terminal color schemes, Cedilla RON themes...).

use cosmic::iced::Color;
use cosmic::iced::core::font::{self, Font};
use cosmic::iced::core::text::highlighter::{self, Format};
use std::ops::Range;
use std::sync::LazyLock;

pub use syntect;
pub use two_face;

use syntect::highlighting;
use syntect::parsing;

/// A reference to a syntect theme that lives for the whole program.
///
/// Themes are loaded once and leaked on purpose, this mirrors how the iced
/// highlighter keeps its themes in a `static` and keeps the highlighter cheap
/// to clone and compare.
pub type ThemeRef = &'static highlighting::Theme;

static SYNTAXES: LazyLock<parsing::SyntaxSet> =
    LazyLock::new(two_face::syntax::extra_no_newlines);

static DEFAULT_THEMES: LazyLock<highlighting::ThemeSet> =
    LazyLock::new(highlighting::ThemeSet::load_defaults);

const LINES_PER_SNAPSHOT: usize = 50;

/// The syntax set used by the highlighter (the extended `two-face` set).
pub fn syntax_set() -> &'static parsing::SyntaxSet {
    &SYNTAXES
}

/// The syntect default theme set (`base16-*`, `InspiredGitHub`, `Solarized`).
pub fn default_theme_set() -> &'static highlighting::ThemeSet {
    &DEFAULT_THEMES
}

/// Returns one of the syntect default themes by key.
pub fn default_theme(key: &str) -> Option<ThemeRef> {
    DEFAULT_THEMES.themes.get(key)
}

/// Leaks a theme so it can be referenced for the rest of the program.
pub fn leak_theme(theme: highlighting::Theme) -> ThemeRef {
    Box::leak(Box::new(theme))
}

/// The settings of a [`Highlighter`].
#[derive(Debug, Clone)]
pub struct Settings {
    /// The theme used for highlighting.
    pub theme: ThemeRef,
    /// The extension of the file or the name of the language to highlight.
    pub token: String,
    /// The base font of the text being highlighted. Bold and italic highlights
    /// keep this family instead of forcing the monospace font.
    pub font: Font,
}

impl PartialEq for Settings {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.theme, other.theme) && self.token == other.token && self.font == other.font
    }
}

/// A syntax highlighter driven by an arbitrary syntect theme.
#[derive(Debug)]
pub struct Highlighter {
    syntax: &'static parsing::SyntaxReference,
    highlighter: highlighting::Highlighter<'static>,
    caches: Vec<(parsing::ParseState, parsing::ScopeStack)>,
    current_line: usize,
    font: Font,
}

impl highlighter::Highlighter for Highlighter {
    type Settings = Settings;
    type Highlight = Highlight;

    type Iterator<'a> = Box<dyn Iterator<Item = (Range<usize>, Self::Highlight)> + 'a>;

    fn new(settings: &Self::Settings) -> Self {
        let syntax = find_syntax(&settings.token);
        let highlighter = highlighting::Highlighter::new(settings.theme);
        let parser = parsing::ParseState::new(syntax);
        let stack = parsing::ScopeStack::new();

        Highlighter {
            syntax,
            highlighter,
            caches: vec![(parser, stack)],
            current_line: 0,
            font: settings.font,
        }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        self.syntax = find_syntax(&new_settings.token);
        self.highlighter = highlighting::Highlighter::new(new_settings.theme);
        self.font = new_settings.font;

        // Restart the highlighter
        self.change_line(0);
    }

    fn change_line(&mut self, line: usize) {
        let snapshot = line / LINES_PER_SNAPSHOT;

        if snapshot <= self.caches.len() {
            self.caches.truncate(snapshot);
            self.current_line = snapshot * LINES_PER_SNAPSHOT;
        } else {
            self.caches.truncate(1);
            self.current_line = 0;
        }

        let (parser, stack) = self.caches.last().cloned().unwrap_or_else(|| {
            (
                parsing::ParseState::new(self.syntax),
                parsing::ScopeStack::new(),
            )
        });

        self.caches.push((parser, stack));
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        if self.current_line / LINES_PER_SNAPSHOT >= self.caches.len() {
            let (parser, stack) = self.caches.last().expect("Caches must not be empty");

            self.caches.push((parser.clone(), stack.clone()));
        }

        self.current_line += 1;

        let (parser, stack) = self.caches.last_mut().expect("Caches must not be empty");

        let ops = parser.parse_line(line, &SYNTAXES).unwrap_or_default();
        let font = self.font;

        Box::new(scope_iterator(ops, line, stack, &self.highlighter, font))
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

fn find_syntax(token: &str) -> &'static parsing::SyntaxReference {
    SYNTAXES
        .find_syntax_by_token(token)
        .unwrap_or_else(|| SYNTAXES.find_syntax_plain_text())
}

fn scope_iterator<'a>(
    ops: Vec<(usize, parsing::ScopeStackOp)>,
    line: &str,
    stack: &'a mut parsing::ScopeStack,
    highlighter: &'a highlighting::Highlighter<'static>,
    font: Font,
) -> impl Iterator<Item = (Range<usize>, Highlight)> + 'a {
    ScopeRangeIterator {
        ops,
        line_length: line.len(),
        index: 0,
        last_str_index: 0,
    }
    .filter_map(move |(range, scope)| {
        let _ = stack.apply(&scope);

        if range.is_empty() {
            None
        } else {
            Some((
                range,
                Highlight {
                    style: highlighter.style_mod_for_stack(&stack.scopes),
                    font,
                },
            ))
        }
    })
}

/// A highlight produced by a [`Highlighter`].
#[derive(Debug)]
pub struct Highlight {
    style: highlighting::StyleModifier,
    font: Font,
}

impl Highlight {
    /// Returns the color of this [`Highlight`].
    pub fn color(&self) -> Option<Color> {
        self.style.foreground.map(to_color)
    }

    /// Returns the font of this [`Highlight`].
    ///
    /// Bold and italic variants are derived from the base font of the editor.
    pub fn font(&self) -> Option<Font> {
        self.style.font_style.and_then(|style| {
            let bold = style.contains(highlighting::FontStyle::BOLD);
            let italic = style.contains(highlighting::FontStyle::ITALIC);

            if bold || italic {
                Some(Font {
                    weight: if bold {
                        font::Weight::Bold
                    } else {
                        self.font.weight
                    },
                    style: if italic {
                        font::Style::Italic
                    } else {
                        self.font.style
                    },
                    ..self.font
                })
            } else {
                None
            }
        })
    }

    /// Returns the [`Format`] of the [`Highlight`].
    pub fn to_format(&self) -> Format<Font> {
        Format {
            color: self.color(),
            font: self.font(),
        }
    }
}

/// Converts a syntect color into an iced color.
pub fn to_color(color: highlighting::Color) -> Color {
    Color::from_rgba8(color.r, color.g, color.b, color.a as f32 / 255.0)
}

/// Background color declared by a theme, if any.
pub fn theme_background(theme: &highlighting::Theme) -> Option<Color> {
    theme.settings.background.map(to_color)
}

/// Foreground color declared by a theme, if any.
pub fn theme_foreground(theme: &highlighting::Theme) -> Option<Color> {
    theme.settings.foreground.map(to_color)
}

/// Whether a theme is dark, judged by the luminance of its background.
pub fn theme_is_dark(theme: &highlighting::Theme) -> bool {
    match theme.settings.background {
        Some(bg) => {
            let luminance =
                (0.2126 * bg.r as f32 + 0.7152 * bg.g as f32 + 0.0722 * bg.b as f32) / 255.0;
            luminance < 0.5
        }
        None => theme
            .name
            .as_deref()
            .is_some_and(|name| name.to_ascii_lowercase().contains("dark")),
    }
}

struct ScopeRangeIterator {
    ops: Vec<(usize, parsing::ScopeStackOp)>,
    line_length: usize,
    index: usize,
    last_str_index: usize,
}

impl Iterator for ScopeRangeIterator {
    type Item = (std::ops::Range<usize>, parsing::ScopeStackOp);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index > self.ops.len() {
            return None;
        }

        let next_str_i = if self.index == self.ops.len() {
            self.line_length
        } else {
            self.ops[self.index].0
        };

        let range = self.last_str_index..next_str_i;
        self.last_str_index = next_str_i;

        let op = if self.index == 0 {
            parsing::ScopeStackOp::Noop
        } else {
            self.ops[self.index - 1].1.clone()
        };

        self.index += 1;
        Some((range, op))
    }
}
