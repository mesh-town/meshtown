//! The file of one added network (`nets/<id>/config.json`): the envelope of the main file plus
//! `created_unix`, and in `config` only the rows of the network block.
//!
//! `parse` reads it, `render` writes the text, `save` puts it on disk.

mod parse;
mod render;
mod save;

pub use parse::{load_net_file, parse_net_text, LoadedNetFile, ParsedNetFile};
pub use render::render_net_file;
pub use save::save_net_file;
