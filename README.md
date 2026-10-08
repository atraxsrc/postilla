<div align="center">
<br>
<img src="./resources/icons/hicolor/scalable/apps/icon.svg" width="150" />
<h1 align="center">Postilla</h1>

<h3>Markdown notes for the COSMIC™ desktop</h3>

<p>A <em>postilla</em> is a note written in the margin. This one is a fork of
<a href="https://github.com/mariinkys/cedilla">Cedilla</a> by Alex Marín, grown into a full notes app.</p>
</div>

## Features

- **Vault** of notes with folders, rename/move, external change detection, and the ability to open any existing folder as the vault
- **Three layouts**: editor only, editor + live preview, preview only (`Ctrl+1` / `Ctrl+2` / `Ctrl+3`, or the switcher in the header bar)
- **Zen mode** (`F11`): hides every bar and the sidebar, leaving only your text
- **Clipboard**: cut / copy / paste / select all from the *Edit* menu, the editor right-click menu or the usual shortcuts; `Ctrl+V` pastes images straight into the note
- **Formatting**: bold, italic, strikethrough, code, links, quotes, tables, lists, headings, rules, date & time stamps (`Format` menu, help bar, or shortcuts such as `Ctrl+B`, `Ctrl+I`, `Ctrl+K`, `Ctrl+Shift+D`)
- **Outline** (`Ctrl+Shift+O`): jump between the headings of the document
- **Search in vault** (`Ctrl+Shift+F`): full text (or regex) search across every note, open a result at the matching line
- **Autosave** with a configurable delay
- **Export** to PDF (rendered offline with the bundled Typst engine, no external service needed), to standalone HTML, or copy as HTML
- **Tray icon** in the panel with *Show* and *Quit*
- Word count, character count and reading time in the status bar
- **Fully customizable syntax highlighting**: import TextMate/Sublime `.tmTheme` files, COSMIC Terminal color schemes or simple RON themes, and optionally paint the editor and the whole window with the theme colors
- **Frosted glass** window effect with adjustable opacity and background color on COSMIC
- **Typst** math and **Mermaid** diagrams inside code blocks

## Install

### From source

You need a Rust toolchain, [just](https://github.com/casey/just) and a few development libraries. On Pop!\_OS / Ubuntu:

```sh
sudo apt install just pkg-config libxkbcommon-dev libwayland-dev libfontconfig-dev libfreetype-dev libexpat1-dev
```

Run it straight from the source tree:

```sh
cargo run --release
```

Or build and install it system wide:

```sh
just build-release
sudo just install
```

### Debian package

```sh
sudo apt install dpkg-dev
cargo install cargo-deb
just deb            # builds target/debian/postilla_<version>_amd64.deb
just install-deb    # builds and installs it with apt
```

The package installs the `postilla` binary, the launcher entry, icons and metainfo, and lists the shared
libraries it needs as dependencies automatically.

### Next to Cedilla

Postilla uses its own application id (`io.github.atraxsrc.Postilla`), so it can be installed and run side by
side with Cedilla: it has its own config, themes folder and default vault. To work on the notes of an existing
Cedilla installation, open *Settings → General → Use another folder as vault* and pick its vault; nothing is
moved or copied. The Cedilla Flatpak keeps its vault in
`~/.var/app/dev.mariinkys.Cedilla/data/dev.mariinkys.Cedilla/vault`.

## Syntax highlighting themes

Postilla ships the usual built-in themes (Inspired GitHub, Ocean, Mocha, Eighties, Solarized...) and loads any
theme placed in its themes folder (*Settings → Appearance → Themes folder*, or use *Import theme…*). Three
formats are detected automatically:

| Format | File | Notes |
| --- | --- | --- |
| **COSMIC Terminal color scheme** | `.ron` | The exact files COSMIC Terminal imports and exports. The 16 ANSI colors are mapped onto syntax scopes (keywords → magenta, strings → green, functions → blue, headings → bright blue…). |
| **Postilla theme** | `.ron` | A flat list of named colors: `keyword`, `string`, `heading`, `link`… plus optional raw TextMate scope overrides. See [`resources/themes/example-postilla-theme.ron`](./resources/themes/example-postilla-theme.ron). |
| **TextMate / Sublime theme** | `.tmTheme` | Any theme from the Sublime Text ecosystem. |

```ron
// A minimal Postilla theme
(
    name: "Mine",
    background: "#1e1e2e",
    foreground: "#cdd6f4",
    comment: "#6c7086",
    keyword: "#cba6f7",
    string: "#a6e3a1",
    heading: "#89b4fa",
    link: "#74c7ec",
    scopes: { "entity.name.function.macro": "#f5c2e7" },
)
```

Pick a theme for light and for dark mode independently. Turn on *Use highlighter theme colors* to paint the
editor, the preview code blocks and the window background with the theme's own colors (great with terminal
themes). A fixed window color can also be set under *Settings → Window*.

## Frosted glass

Under *Settings → Window* you can set the frosted glass behaviour (*Follow desktop*, *Enabled*, *Disabled*),
the window opacity and the background color. Blur behind the window is provided by the compositor, so it needs
COSMIC with blur support; on other desktops the window simply becomes translucent.

## Typst and Mermaid

Code blocks with the `typst` or `typ` language render as [Typst](https://typst.app/), which is handy for math
and formulas, and blocks with `mermaid` or `mmd` render as [Mermaid](https://mermaid.js.org/intro/) diagrams
(powered by [merman](https://github.com/Latias94/merman)). Both are also rendered in PDF and HTML exports.

## Acknowledgements

- Postilla is a fork of [Cedilla](https://github.com/mariinkys/cedilla), Copyright 2026 © Alex Marín
  (mariinkys), licensed under the GPL-3.0. Most of the foundation of this app is his work.
- Markdown and HTML rendering uses [Frostmark](https://github.com/Mrmayman/frostmark) by Mrmayman, adapted for
  libcosmic.
- "[Pop Icons](http://github.com/pop-os/icon-theme)" by [System76](http://system76.com/) is licensed under
  [CC-SA-4.0](http://creativecommons.org/licenses/by-sa/4.0/).
- Built with [libcosmic](https://github.com/pop-os/libcosmic), [Typst](https://typst.app/) and
  [syntect](https://github.com/trishume/syntect).

## License

Released under the terms of the [GPL-3.0](./LICENSE). Postilla's changes are Copyright 2026 © Postilla
contributors; the original work remains Copyright 2026 © Alex Marín.
