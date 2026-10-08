use std::collections::HashMap;
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

// Help from: https://github.com/tfachmann/typst-as-library/blob/main/src/lib.rs

pub struct MinimalWorld {
    library: typst::utils::LazyHash<Library>,
    book: typst::utils::LazyHash<FontBook>,
    fonts: Vec<Font>,
    source: Source,
    /// Extra files (images...) addressable from the document by their
    /// project relative path, e.g. `#image("img/photo.png")`.
    files: HashMap<String, Bytes>,
}

impl MinimalWorld {
    pub fn new(source: &str) -> Self {
        Self::with_files(source, HashMap::new())
    }

    /// A world for `source` that can also serve the given files.
    pub fn with_files(source: &str, files: HashMap<String, Vec<u8>>) -> Self {
        let mut fonts = Vec::new();
        let mut book = FontBook::new();

        for font_data in typst_assets::fonts() {
            let font_bytes = Bytes::new(font_data);
            for font in Font::iter(font_bytes) {
                book.push(font.info().clone());
                fonts.push(font);
            }
        }

        Self {
            library: typst::utils::LazyHash::new(Library::builder().build()),
            book: typst::utils::LazyHash::new(book),
            fonts,
            source: Source::detached(source),
            files: files
                .into_iter()
                .map(|(path, bytes)| (normalize_path(&path), Bytes::new(bytes)))
                .collect(),
        }
    }
}

fn normalize_path(path: &str) -> String {
    path.trim_start_matches("./").trim_start_matches('/').to_string()
}

impl World for MinimalWorld {
    fn library(&self) -> &typst::utils::LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &typst::utils::LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.source.id()
    }

    fn source(&self, id: FileId) -> typst::diag::FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(typst::diag::FileError::NotFound("<embedded>".into()))
        }
    }

    fn file(&self, id: FileId) -> typst::diag::FileResult<Bytes> {
        let path = id.vpath().get_without_slash().to_string();
        self.files
            .get(&normalize_path(&path))
            .cloned()
            .ok_or_else(|| typst::diag::FileError::NotFound(path.into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

/// Compiles a complete Typst document to PDF.
///
/// `files` are the extra files (images) the document references, keyed by
/// the path used in the document.
pub fn compile_pdf(source: &str, files: HashMap<String, Vec<u8>>) -> Result<Vec<u8>, String> {
    let world = MinimalWorld::with_files(source, files);

    let compiled = typst::compile::<PagedDocument>(&world);
    let document = compiled.output.map_err(|errors| {
        errors
            .iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|errors| {
        errors
            .iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })
}
