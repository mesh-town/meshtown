//! Compresses the embedded pages (`web_gui/ui/index.html`, `css/*.css`, `bundle/app.js`) with gzip
//! into ``$OUT_DIR/ui/<path>.gz``; `web_gui/routes/pages.rs` embeds the results with `include_bytes!`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::{Compression, GzBuilder};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("ui");
    let ui = manifest.join("web_gui/ui");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=web_gui/ui/index.html");
    println!("cargo:rerun-if-changed=web_gui/ui/css");
    println!("cargo:rerun-if-changed=web_gui/ui/bundle");
    let mut files = vec![ui.join("index.html"), ui.join("bundle/app.js")];
    files.extend(css_files(&ui.join("css")));
    for file in files {
        let rel = file.strip_prefix(&ui).expect("file under ui/");
        compress(&file, &out.join(rel));
    }
}

fn css_files(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let paths = entries.map(|e| e.expect("directory entry").path());
    paths
        .filter(|p| p.extension().is_some_and(|x| x == "css"))
        .collect()
}

/// Writes `src` gzip-compressed to `dst` plus the `.gz` suffix. The header has no file name
/// and a zero time, so the output depends only on the content.
fn compress(src: &Path, dst: &Path) {
    let data = fs::read(src).unwrap_or_else(|e| panic!("{}: {e}", src.display()));
    let mut enc = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    enc.write_all(&data)
        .unwrap_or_else(|e| panic!("{}: {e}", src.display()));
    let gz = enc
        .finish()
        .unwrap_or_else(|e| panic!("{}: {e}", src.display()));
    let mut name = dst.as_os_str().to_owned();
    name.push(".gz");
    let target = PathBuf::from(name);
    let dir = target.parent().expect("parent directory");
    fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    fs::write(&target, gz).unwrap_or_else(|e| panic!("{}: {e}", target.display()));
}
