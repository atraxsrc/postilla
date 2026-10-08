//
// Original code by: Mrmayman <navneetkrishna22@gmail.com>
// https://github.com/Mrmayman/frostmark
//

#![allow(clippy::collapsible_if)]

mod renderer;
mod state;
mod structs;
mod style;
mod typst_world;
mod widgets;

pub use state::MarkState;
pub use structs::{ImageInfo, MarkWidget, RubyMode, UpdateMsg};
pub use style::Style;

// not very clean but the easies way to get pdf export working for mermaid too
pub use renderer::mermaid::render_mermaid_png;
pub use renderer::typst::render_typst_png;
pub use typst_world::compile_pdf;
