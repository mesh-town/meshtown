//! Compresses the embedded pages (`web_gui/ui/index.html`, `css/*.css`, `bundle/app.js`) with gzip
//! into ``$OUT_DIR/ui/<path>.gz``; `web_gui/routes/pages.rs` embeds the results with `include_bytes!`.
//! The word list goes to ``$OUT_DIR/wordlist/words.txt.gz`` without the dice column.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::{Compression, GzBuilder};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let out = out_dir.join("ui");
    let ui = manifest.join("web_gui/ui");
    println!("cargo:rerun-if-changed=build.rs");
    emit_build_version(&git_dir(&manifest));
    println!("cargo:rerun-if-changed=web_gui/ui/index.html");
    println!("cargo:rerun-if-changed=web_gui/ui/css");
    println!("cargo:rerun-if-changed=web_gui/ui/bundle");
    let mut files = vec![ui.join("index.html"), ui.join("bundle/app.js")];
    files.extend(css_files(&ui.join("css")));
    let words_src = manifest.join("members/wordlist/eff_large_wordlist.txt");
    println!("cargo:rerun-if-changed=members/wordlist/eff_large_wordlist.txt");
    compress_words(&words_src, &out_dir.join("wordlist/words.txt.gz"));
    for file in files {
        let rel = file.strip_prefix(&ui).expect("file under ui/");
        compress(&file, &out.join(rel));
    }
}

/// The directory to ask git in: the parent of the real (symlinks resolved) `build.rs`, falling back to
/// `manifest`. A build tree made of links (a farm whose `core/` is a plain directory with a linked `build.rs`)
/// has no repository of its own, and git would climb to an unrelated one above it.
fn git_dir(manifest: &Path) -> PathBuf {
    let real = fs::canonicalize(manifest.join("build.rs")).ok();
    real.and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| manifest.to_path_buf())
}

/// Sets `MESHTOWN_BUILD_VERSION` to `<package version> (<short commit hash>)`, the hash being that of the
/// repository holding the real `build.rs` (`dir`); without git, a repository or a working command it is the
/// package version alone and the build goes on.
fn emit_build_version(dir: &Path) {
    let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let text = match git_output(dir, &["rev-parse", "--short", "HEAD"]) {
        Some(hash) => format!("{version} ({hash})"),
        None => version,
    };
    println!("cargo:rustc-env=MESHTOWN_BUILD_VERSION={text}");
    watch_head(dir);
}

/// Re-runs the build when the checked-out commit changes: the `HEAD` file and the ref it points to. Only
/// existing paths are printed, since a missing one would make the script run on every build.
fn watch_head(manifest: &Path) {
    let mut paths = vec![
        git_path(manifest, "HEAD"),
        git_path(manifest, "packed-refs"),
    ];
    if let Some(branch) = git_output(manifest, &["symbolic-ref", "-q", "HEAD"]) {
        paths.push(git_path(manifest, &branch));
    }
    for path in paths.into_iter().flatten().filter(|p| p.exists()) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn git_path(manifest: &Path, name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(git_output(manifest, &["rev-parse", "--git-path", name])?);
    Some(if path.is_absolute() {
        path
    } else {
        manifest.join(path)
    })
}

/// The trimmed stdout of `git <args>` run in `dir`; `None` if git is missing, fails or prints nothing.
fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.trim();
    (out.status.success() && !text.is_empty()).then(|| text.to_string())
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
    let mut name = dst.as_os_str().to_owned();
    name.push(".gz");
    write_gz(&data, &PathBuf::from(name));
}

/// Writes the second column (the word) of the dice list, one word per line and no trailing
/// newline, gzip-compressed to `dst`.
fn compress_words(src: &Path, dst: &Path) {
    let text = fs::read_to_string(src).unwrap_or_else(|e| panic!("{}: {e}", src.display()));
    let words: Vec<&str> = text
        .lines()
        .filter_map(|line| line.split('\t').nth(1))
        .map(str::trim)
        .collect();
    write_gz(words.join("\n").as_bytes(), dst);
}

fn write_gz(data: &[u8], target: &Path) {
    let mut enc = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    enc.write_all(data)
        .unwrap_or_else(|e| panic!("{}: {e}", target.display()));
    let gz = enc
        .finish()
        .unwrap_or_else(|e| panic!("{}: {e}", target.display()));
    let dir = target.parent().expect("parent directory");
    fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    fs::write(target, gz).unwrap_or_else(|e| panic!("{}: {e}", target.display()));
}
