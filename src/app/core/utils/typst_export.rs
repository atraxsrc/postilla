// SPDX-License-Identifier: GPL-3.0

//! Offline PDF export: the markdown document is translated to Typst markup
//! and compiled with the Typst engine that already ships with Cedilla.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anywho::anywho;
use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Widest an image may be before it is scaled to the text width (in pt)
const MAX_IMAGE_WIDTH_PT: f32 = 440.0;

/// Exports the document to a PDF file at `destination`.
pub async fn export_pdf(
    file_path: Option<PathBuf>,
    content: String,
    destination: String,
) -> Result<(), anywho::Error> {
    let (title, base_dir) = match &file_path {
        Some(path) => (
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Document")
                .to_string(),
            path.parent().unwrap_or(Path::new(".")).to_path_buf(),
        ),
        None => ("Document".to_string(), PathBuf::from(".")),
    };

    let images = load_images(&content, &base_dir).await;

    let pdf = tokio::task::spawn_blocking(move || {
        let (source, files) = markdown_to_typst(&content, &title, &images);
        frostmark::compile_pdf(&source, files)
    })
    .await
    .map_err(|e| anywho!("{e}"))?
    .map_err(|e| anywho!("PDF rendering failed: {e}"))?;

    tokio::fs::write(destination, pdf)
        .await
        .map_err(|e| anywho!("{e}"))?;

    Ok(())
}

/// Image bytes keyed by the url used in the document
type ImageMap = HashMap<String, (String, Vec<u8>)>;

fn parser_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SUPERSCRIPT
        | Options::ENABLE_SUBSCRIPT
}

/// Collects every image of the document and loads it (local file or download).
async fn load_images(content: &str, base_dir: &Path) -> ImageMap {
    let mut urls: Vec<String> = Vec::new();
    for event in Parser::new_ext(content, parser_options()) {
        match event {
            Event::Start(Tag::Image { dest_url, .. }) => urls.push(dest_url.to_string()),
            Event::Html(html) | Event::InlineHtml(html) => {
                urls.extend(html_image_sources(&html));
            }
            _ => {}
        }
    }
    urls.sort();
    urls.dedup();

    let mut images = ImageMap::new();
    for (index, url) in urls.into_iter().enumerate() {
        if let Some((ext, bytes)) = load_image(&url, base_dir).await {
            images.insert(url, (format!("img/image-{index}.{ext}"), bytes));
        }
    }
    images
}

async fn load_image(url: &str, base_dir: &Path) -> Option<(String, Vec<u8>)> {
    let (bytes, hint) = if url.starts_with("http://") || url.starts_with("https://") {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .ok()?;
        let response = client.get(url).send().await.ok()?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        (response.bytes().await.ok()?.to_vec(), content_type)
    } else {
        let path = url.strip_prefix("file://").unwrap_or(url);
        let path = percent_encoding::percent_decode_str(path).decode_utf8_lossy();
        let path = Path::new(path.as_ref());
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base_dir.join(path)
        };
        (tokio::fs::read(&path).await.ok()?, None)
    };

    let ext = image_extension(&bytes, hint.as_deref(), url)?;
    Some((ext, bytes))
}

/// Detects the image format (typst supports png, jpg, gif, svg, webp).
fn image_extension(bytes: &[u8], content_type: Option<&str>, url: &str) -> Option<String> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some("png".into());
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg".into());
    }
    if bytes.starts_with(b"GIF8") {
        return Some("gif".into());
    }
    if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp".into());
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
    if head.contains("<svg") {
        return Some("svg".into());
    }
    if content_type.is_some_and(|c| c.contains("svg")) {
        return Some("svg".into());
    }
    let ext = url
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => Some(ext),
        _ => None,
    }
}

/// `src` attributes of `<img>` tags in an HTML fragment
fn html_image_sources(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find("<img") {
        let tag_start = from + pos;
        let tag_end = lower[tag_start..]
            .find('>')
            .map(|e| tag_start + e)
            .unwrap_or(lower.len());
        let tag = &html[tag_start..tag_end];
        if let Some(src) = attribute(tag, "src") {
            out.push(src);
        }
        from = tag_end;
    }
    out
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let pos = lower.find(&format!("{name}="))? + name.len() + 1;
    let rest = &tag[pos..];
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let end = rest[1..].find(quote)?;
        Some(rest[1..1 + end].to_string())
    } else {
        Some(rest.split_whitespace().next().unwrap_or("").to_string())
    }
}

/// Translates markdown to a complete Typst document. Returns the source and
/// the files (images) it references.
pub fn markdown_to_typst(
    content: &str,
    title: &str,
    images: &ImageMap,
) -> (String, HashMap<String, Vec<u8>>) {
    let mut files: HashMap<String, Vec<u8>> = images
        .values()
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect();

    // footnote definitions are needed before the references that use them
    let footnotes = collect_footnotes(content, images, &mut files);

    let mut writer = TypstWriter::new(images, footnotes, files);
    for event in Parser::new_ext(content, parser_options()) {
        writer.event(event);
    }
    let (body, files) = writer.finish();

    let source = format!(
        "{}\n#set document(title: \"{}\")\n\n{body}",
        preamble(),
        typst_string(title)
    );

    (source, files)
}

fn preamble() -> &'static str {
    r##"#set page(paper: "a4", margin: 2cm, numbering: "1")
#set text(font: ("Libertinus Serif", "New Computer Modern"), size: 11pt)
#set par(justify: false, leading: 0.65em)
#show heading.where(level: 1): set text(size: 1.7em)
#show heading.where(level: 2): set text(size: 1.4em)
#show heading.where(level: 3): set text(size: 1.2em)
#show heading: set block(above: 1.4em, below: 0.8em)
#show link: set text(fill: rgb("#1c71d8"))
#show raw: set text(font: "DejaVu Sans Mono", size: 0.9em)
#show raw.where(block: true): it => block(fill: luma(246), stroke: 0.5pt + luma(215), inset: 8pt, radius: 4pt, width: 100%, it)
#show raw.where(block: false): it => box(fill: luma(240), inset: (x: 3pt, y: 0pt), outset: (y: 3pt), radius: 2pt, it)
#set table(stroke: 0.5pt + luma(190), inset: 6pt)
#show table.cell.where(y: 0): strong
#show quote.where(block: true): it => block(inset: (left: 1em, y: 0.5em), stroke: (left: 2pt + luma(190)), it.body)
#set list(indent: 0.5em)
#set enum(indent: 0.5em)"##
}

fn collect_footnotes(
    content: &str,
    images: &ImageMap,
    files: &mut HashMap<String, Vec<u8>>,
) -> HashMap<String, String> {
    let mut definitions = HashMap::new();
    let mut current: Option<(String, TypstWriter)> = None;

    for event in Parser::new_ext(content, parser_options()) {
        match event {
            Event::Start(Tag::FootnoteDefinition(label)) => {
                current = Some((
                    label.to_string(),
                    TypstWriter::new(images, HashMap::new(), files.clone()),
                ));
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                if let Some((label, writer)) = current.take() {
                    let (body, _) = writer.finish();
                    definitions.insert(label, body.trim().to_string());
                }
            }
            other => {
                if let Some((_, writer)) = current.as_mut() {
                    writer.event(other);
                }
            }
        }
    }

    definitions
}

/// Escapes text so it is rendered literally in Typst markup
pub fn typst_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '\\' | '*' | '_' | '`' | '#' | '@' | '<' | '>' | '[' | ']' | '$' | '~' | '/' | '-'
            | '+' | '=' | '\'' | '"' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Escapes text for use inside a Typst string literal
pub fn typst_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListKind {
    Bullet,
    Numbered(u64),
}

struct Table {
    alignments: Vec<Alignment>,
    cells: Vec<String>,
    in_head: bool,
    head_cells: usize,
}

struct TypstWriter<'a> {
    out: String,
    images: &'a ImageMap,
    footnotes: HashMap<String, String>,
    files: HashMap<String, Vec<u8>>,
    /// Nested lists, innermost last
    lists: Vec<ListKind>,
    /// Buffers for content inside brackets (quotes, cells, links...)
    stack: Vec<String>,
    table: Option<Table>,
    code_block: Option<String>,
    code_buffer: String,
    /// Index of the next mermaid diagram (for unique file names)
    mermaid_index: usize,
    /// Pending link text destination
    links: Vec<String>,
    /// Alt text of the image being described
    image: Option<(String, String)>,
    /// Whether the current list is tight (no blank lines between items)
    item_open: bool,
}

impl<'a> TypstWriter<'a> {
    fn new(
        images: &'a ImageMap,
        footnotes: HashMap<String, String>,
        files: HashMap<String, Vec<u8>>,
    ) -> Self {
        Self {
            out: String::new(),
            images,
            footnotes,
            files,
            lists: Vec::new(),
            stack: Vec::new(),
            table: None,
            code_block: None,
            code_buffer: String::new(),
            mermaid_index: 0,
            links: Vec::new(),
            image: None,
            item_open: false,
        }
    }

    fn finish(self) -> (String, HashMap<String, Vec<u8>>) {
        (self.out, self.files)
    }

    /// Current output target (innermost bracket buffer or the document)
    fn buf(&mut self) -> &mut String {
        match self.stack.last_mut() {
            Some(buf) => buf,
            None => &mut self.out,
        }
    }

    fn push(&mut self, s: &str) {
        self.buf().push_str(s);
    }

    fn indent(&self) -> String {
        "  ".repeat(self.lists.len())
    }

    /// Starts a block level element on its own line
    fn block_start(&mut self) {
        let indent = self.indent();
        let buf = self.buf();
        if !buf.is_empty() && !buf.ends_with('\n') {
            buf.push('\n');
        }
        if !buf.is_empty() && !buf.ends_with("\n\n") && indent.is_empty() {
            buf.push('\n');
        }
        buf.push_str(&indent);
    }

    fn block_end(&mut self) {
        let buf = self.buf();
        if !buf.ends_with('\n') {
            buf.push('\n');
        }
        buf.push('\n');
    }

    fn event(&mut self, event: Event<'_>) {
        // inside a fenced code block everything is source text
        if self.code_block.is_some() {
            match event {
                Event::Text(text) => self.code_buffer.push_str(&text),
                Event::End(TagEnd::CodeBlock) => self.end_code_block(),
                _ => {}
            }
            return;
        }

        // image alt text is collected, not rendered
        if let Some((_, alt)) = self.image.as_mut() {
            match event {
                Event::Text(text) | Event::Code(text) => alt.push_str(&text),
                Event::End(TagEnd::Image) => self.end_image(),
                _ => {}
            }
            return;
        }

        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                let escaped = typst_text(&text);
                self.push(&escaped);
            }
            Event::Code(code) => {
                let s = format!("#raw(\"{}\")", typst_string(&code));
                self.push(&s);
            }
            Event::InlineMath(math) => {
                let s = format!("#raw(\"{}\")", typst_string(&math));
                self.push(&s);
            }
            Event::DisplayMath(math) => {
                self.block_start();
                let s = format!("#raw(block: true, \"{}\")", typst_string(&math));
                self.push(&s);
                self.block_end();
            }
            Event::Html(html) => self.html(&html, true),
            Event::InlineHtml(html) => self.html(&html, false),
            Event::FootnoteReference(label) => {
                let body = self
                    .footnotes
                    .get(label.as_ref())
                    .cloned()
                    .unwrap_or_else(|| typst_text(&label));
                let s = format!("#footnote[{body}]");
                self.push(&s);
            }
            Event::SoftBreak => self.push(" "),
            Event::HardBreak => self.push(" \\\n"),
            Event::Rule => {
                self.block_start();
                self.push("#line(length: 100%, stroke: 0.5pt + luma(190))");
                self.block_end();
            }
            Event::TaskListMarker(checked) => {
                self.push(if checked { "#sym.ballot.check " } else { "#sym.ballot " });
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                if self.item_open {
                    // first paragraph of a list item stays on the marker line
                    self.item_open = false;
                } else if self.table.is_none() {
                    self.block_start();
                }
            }
            Tag::Heading { level, .. } => {
                self.block_start();
                let marks = match level {
                    HeadingLevel::H1 => "=",
                    HeadingLevel::H2 => "==",
                    HeadingLevel::H3 => "===",
                    HeadingLevel::H4 => "====",
                    HeadingLevel::H5 => "=====",
                    HeadingLevel::H6 => "======",
                };
                self.push(marks);
                self.push(" ");
            }
            Tag::BlockQuote(_) => {
                self.block_start();
                self.push("#quote(block: true)[");
                self.stack.push(String::new());
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.trim().to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code_block = Some(lang);
                self.code_buffer.clear();
            }
            Tag::List(start) => {
                let kind = match start {
                    Some(n) => ListKind::Numbered(n),
                    None => ListKind::Bullet,
                };
                if self.lists.is_empty() {
                    self.block_start();
                } else if !self.buf().ends_with('\n') {
                    self.push("\n");
                }
                self.lists.push(kind);
            }
            Tag::Item => {
                let depth = self.lists.len().saturating_sub(1);
                let marker = match self.lists.last_mut() {
                    Some(ListKind::Numbered(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "- ".to_string(),
                };
                let line = format!("{}{marker}", "  ".repeat(depth));
                if !self.buf().ends_with('\n') {
                    self.push("\n");
                }
                self.push(&line);
                self.item_open = true;
            }
            Tag::Table(alignments) => {
                self.block_start();
                self.table = Some(Table {
                    alignments,
                    cells: Vec::new(),
                    in_head: false,
                    head_cells: 0,
                });
            }
            Tag::TableHead => {
                if let Some(table) = self.table.as_mut() {
                    table.in_head = true;
                }
            }
            Tag::TableRow => {}
            Tag::TableCell => self.stack.push(String::new()),
            Tag::Emphasis => self.push("#emph["),
            Tag::Strong => self.push("#strong["),
            Tag::Strikethrough => self.push("#strike["),
            Tag::Superscript => self.push("#super["),
            Tag::Subscript => self.push("#sub["),
            Tag::Link { dest_url, .. } => {
                self.links.push(dest_url.to_string());
                let s = format!("#link(\"{}\")[", typst_string(&dest_url));
                self.push(&s);
            }
            Tag::Image { dest_url, .. } => {
                self.image = Some((dest_url.to_string(), String::new()));
            }
            Tag::FootnoteDefinition(_)
            | Tag::HtmlBlock
            | Tag::MetadataBlock(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                if self.table.is_none() {
                    if self.lists.is_empty() {
                        self.block_end();
                    } else {
                        self.push("\n");
                    }
                }
            }
            TagEnd::Heading(_) => self.block_end(),
            TagEnd::BlockQuote(_) => {
                let body = self.stack.pop().unwrap_or_default();
                self.push(body.trim());
                self.push("]");
                self.block_end();
            }
            TagEnd::CodeBlock => self.end_code_block(),
            TagEnd::List(_) => {
                self.lists.pop();
                self.item_open = false;
                if self.lists.is_empty() {
                    self.block_end();
                }
            }
            TagEnd::Item => {
                self.item_open = false;
                if !self.buf().ends_with('\n') {
                    self.push("\n");
                }
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    let columns = table.alignments.len().max(1);
                    let align = table
                        .alignments
                        .iter()
                        .map(|a| match a {
                            Alignment::Center => "center",
                            Alignment::Right => "right",
                            Alignment::None | Alignment::Left => "left",
                        })
                        .collect::<Vec<_>>()
                        .join(", ");

                    let mut s = format!("#table(\n  columns: {columns},\n  align: ({align},),\n");
                    let (head, body) = table.cells.split_at(table.head_cells.min(table.cells.len()));
                    if !head.is_empty() {
                        s.push_str("  table.header(");
                        s.push_str(
                            &head
                                .iter()
                                .map(|c| format!("[{c}]"))
                                .collect::<Vec<_>>()
                                .join(", "),
                        );
                        s.push_str("),\n");
                    }
                    for cell in body {
                        s.push_str(&format!("  [{cell}],\n"));
                    }
                    s.push(')');
                    self.push(&s);
                    self.block_end();
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = self.table.as_mut() {
                    table.in_head = false;
                    table.head_cells = table.cells.len();
                }
            }
            TagEnd::TableRow => {}
            TagEnd::TableCell => {
                let cell = self.stack.pop().unwrap_or_default();
                if let Some(table) = self.table.as_mut() {
                    table.cells.push(cell.trim().to_string());
                }
            }
            TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Superscript
            | TagEnd::Subscript => self.push("]"),
            TagEnd::Link => {
                self.links.pop();
                self.push("]");
            }
            TagEnd::Image => self.end_image(),
            TagEnd::FootnoteDefinition
            | TagEnd::HtmlBlock
            | TagEnd::MetadataBlock(_)
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition => {}
        }
    }

    fn end_code_block(&mut self) {
        let lang = self.code_block.take().unwrap_or_default();
        let source = std::mem::take(&mut self.code_buffer);
        let source = source.trim_end_matches('\n');

        self.block_start();
        match lang.as_str() {
            "typst" | "typ" => {
                // Typst markup is embedded as is, isolated in a block
                let s = format!("#block(width: 100%)[\n{source}\n]");
                self.push(&s);
            }
            "mermaid" | "mmd" => match frostmark::render_mermaid_png(source, false, 3.0) {
                Ok((bytes, width)) => {
                    let path = format!("img/mermaid-{}.png", self.mermaid_index);
                    self.mermaid_index += 1;
                    self.files.insert(path.clone(), bytes);
                    let s = image_markup(&path, Some(width / 3.0));
                    self.push(&s);
                }
                Err(_) => {
                    let s = format!("#raw(block: true, \"{}\")", typst_string(source));
                    self.push(&s);
                }
            },
            _ => {
                let lang_arg = if lang.is_empty() {
                    String::new()
                } else {
                    format!("lang: \"{}\", ", typst_string(&lang))
                };
                let s = format!("#raw(block: true, {lang_arg}\"{}\")", typst_string(source));
                self.push(&s);
            }
        }
        self.block_end();
    }

    fn end_image(&mut self) {
        let Some((url, alt)) = self.image.take() else {
            return;
        };
        self.image_by_url(&url, &alt);
    }

    fn image_by_url(&mut self, url: &str, alt: &str) {
        match self.images.get(url) {
            Some((path, _)) => {
                let s = image_markup(path, None);
                self.push(&s);
            }
            None => {
                // unavailable image: show its alt text (or url) in italics
                let label = if alt.trim().is_empty() { url } else { alt };
                let s = format!("#emph[{}]", typst_text(label));
                self.push(&s);
            }
        }
    }

    fn html(&mut self, html: &str, block: bool) {
        let lower = html.to_ascii_lowercase();
        if lower.contains("<br") {
            self.push(" \\\n");
        }
        for src in html_image_sources(html) {
            if block {
                self.block_start();
            }
            let alt = attribute(html, "alt").unwrap_or_default();
            self.image_by_url(&src, &alt);
            if block {
                self.block_end();
            }
        }
        // any other HTML is dropped, Typst can't render it
    }
}

fn image_markup(path: &str, width_pt: Option<f32>) -> String {
    match width_pt {
        Some(width) if width < MAX_IMAGE_WIDTH_PT => {
            format!("#align(center)[#image(\"{path}\", width: {width:.1}pt)]")
        }
        Some(_) => format!("#align(center)[#image(\"{path}\", width: 100%)]"),
        None => format!("#image(\"{path}\", width: 100%)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markup() {
        assert_eq!(typst_text("a_b*c#d"), "a\\_b\\*c\\#d");
        assert_eq!(typst_string("say \"hi\"\n"), "say \\\"hi\\\"\\n");
    }

    #[test]
    fn converts_common_markdown() {
        let md = "# Title\n\nSome *text* with `code` and a [link](https://x.y).\n\n- one\n- [x] two\n\n1. first\n2. second\n\n> quoted\n\n```rust\nfn main() {}\n```\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\n---\n";
        let (source, files) = markdown_to_typst(md, "Doc", &ImageMap::new());
        assert!(files.is_empty());
        assert!(source.contains("= Title"));
        assert!(source.contains("#emph[text]"));
        assert!(source.contains("#raw(\"code\")"));
        assert!(source.contains("#link(\"https://x.y\")[link]"));
        assert!(source.contains("- one"));
        assert!(source.contains("#sym.ballot.check two"));
        assert!(source.contains("1. first"));
        assert!(source.contains("2. second"));
        assert!(source.contains("#quote(block: true)[quoted]"));
        assert!(source.contains("#raw(block: true, lang: \"rust\", \"fn main() {}\")"));
        assert!(source.contains("table.header([a], [b])"));
        assert!(source.contains("#line(length: 100%"));
    }

    #[test]
    fn compiles_to_pdf() {
        let md = "# Hello\n\nA paragraph with **bold** text, a snake_case word and 1 + 1 = 2.\n\n- item\n  - nested\n\n```typst\n$ x^2 + y^2 = z^2 $\n```\n";
        let (source, files) = markdown_to_typst(md, "Hello", &ImageMap::new());
        let pdf = frostmark::compile_pdf(&source, files).expect("document compiles");
        assert!(pdf.starts_with(b"%PDF"));
    }
}
