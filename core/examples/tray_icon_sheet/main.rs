//! Example: draws the tray icon with this branch's real drawing code and writes a PNG to look at
//! by eye, without a menu bar.
//!
//! What it does: for 14 ring compositions (the last two: a grey ring of 8 with sector 0 and 3 lit) calls `ring_rgba_lit` (the same raster the tray sets) at 36 px
//! and 22 px, writes each icon and its enlargement to a 256×256 slot with nearest neighbor (an integer
//! factor: 7× for 36 px, 11× for 22 px, centered on a transparent canvas), then two contact sheets:
//! `sheet-light.png` and `sheet-dark.png` (a light and a dark menu bar). All PNGs are transparent,
//! except the sheets.
//! What it doesn't do: it doesn't change the drawing code and doesn't copy it: the files `core/gui/tray/app/{view_ring,
//! view_ring_paint,view,exit_menu}.rs` are included in the example as is (`#[path]`), because
//! they live in the binary, not in a library. This is an example, not a test: it is not part of the `cargo test`
//! run and is run by hand.
//! Links: run with `cargo run --example tray_icon_sheet [directory]`; by default
//! it writes to `.tmp/tray-sheets/` at the tree root. `image.rs` is the raster, `png.rs` writes PNG,
//! `sheet.rs` is the contact sheets.
//!
//! Case order (numbers in the sheets and file prefixes): 01-05 — 2, 3, 5, 8, 9 members
//! online; 06, 07 — 16 and 17 online (no gaps from 17 on); 08 — six, two offline;
//! 09 — six with all roles (exit, two exit via us, two online, one gray);
//! 10 — six offline; 11 — one member; 12 — zero (the ring is empty, the raster is transparent;
//! in production the icon for zero is solid: `ring_to_draw` returns `None`).

mod image;
mod png;
mod sheet;

mod app;

// The included tray files address each other and the library as `crate::...`;
// these lines provide those paths in the example crate.
mod gui {
    pub(crate) mod tray {
        pub(crate) use crate::app;
    }
}
use meshtown_core::{node, web_gui};

use std::path::{Path, PathBuf};
use std::{env, fs, io};

use app::view_ring::{Ring, RingRole};
use app::view_ring_paint::ring_rgba_lit;
use image::Image;
use sheet::{Entry, Theme, SLOT};

/// Icon sides: 36 is how the tray draws now, 22 is a thin ring.
const SIDES: [u32; 2] = [36, 22];

const LIGHT: Theme = Theme {
    bar: [240, 240, 240, 255],
    gutter: [200, 200, 200, 255],
    ink: [30, 30, 30, 255],
};
const DARK: Theme = Theme {
    bar: [38, 38, 40, 255],
    gutter: [8, 8, 8, 255],
    ink: [225, 225, 225, 255],
};

/// A case: a name for the files and the ring composition.
struct Case {
    slug: &'static str,
    ring: Ring,
    /// The highlighted sector, if any.
    lit: Option<usize>,
}

/// A ring from pairs "role, how many members".
fn ring_of(parts: &[(RingRole, usize)]) -> Ring {
    let add_n = |ring: Ring, &(role, n): &(RingRole, usize)| (0..n).fold(ring, |r, _| r.add(role));
    parts.iter().fold(Ring::EMPTY, add_n)
}

fn online(n: usize) -> Case {
    let slug = ["n02", "n03", "n05", "n08", "n09", "n16", "n17"];
    let pick = [2, 3, 5, 8, 9, 16, 17].iter().position(|&k| k == n);
    Case {
        slug: slug[pick.unwrap_or(0)],
        ring: ring_of(&[(RingRole::Online, n)]),
        lit: None,
    }
}

/// All cases in sheet order.
fn cases() -> Vec<Case> {
    let mut all: Vec<Case> = [2, 3, 5, 8, 9, 16, 17].into_iter().map(online).collect();
    all.push(Case {
        slug: "six-2grey",
        ring: ring_of(&[(RingRole::Online, 4), (RingRole::Offline, 2)]),
        lit: None,
    });
    all.push(Case {
        slug: "six-roles",
        ring: ring_of(&[
            (RingRole::ActiveExit, 1),
            (RingRole::ExitClient, 2),
            (RingRole::Online, 2),
            (RingRole::Offline, 1),
        ]),
        lit: None,
    });
    all.push(Case {
        slug: "six-all-grey",
        ring: ring_of(&[(RingRole::Offline, 6)]),
        lit: None,
    });
    all.push(Case {
        slug: "one",
        ring: ring_of(&[(RingRole::Online, 1)]),
        lit: None,
    });
    all.push(Case {
        slug: "zero",
        ring: Ring::EMPTY,
        lit: None,
    });
    for (slug, k) in [("spin-lit0", 0), ("spin-lit3", 3)] {
        all.push(Case {
            slug,
            ring: ring_of(&[(RingRole::Offline, 8)]),
            lit: Some(k),
        });
    }
    all
}

fn write_png(dir: &Path, name: &str, img: &Image) -> io::Result<()> {
    fs::write(dir.join(format!("{name}.png")), png::encode(img))
}

/// Icons of a case on both sides: writes four files, returns the rasters for the sheets.
fn write_case(dir: &Path, index: usize, case: &Case) -> io::Result<Entry> {
    let mut small = Vec::new();
    let mut zoomed = Vec::new();
    for side in SIDES {
        let raw = Image::square(side, ring_rgba_lit(&case.ring, case.lit, side));
        let big = raw.scaled(SLOT / side).centered(SLOT);
        let stem = format!("{:02}-{}-{side}", index + 1, case.slug);
        write_png(dir, &stem, &raw)?;
        write_png(dir, &format!("{stem}-x256"), &big)?;
        small.push(raw);
        zoomed.push(big);
    }
    Ok(Entry {
        small: small.try_into().unwrap_or_else(|_| unreachable!()),
        zoomed: zoomed.try_into().unwrap_or_else(|_| unreachable!()),
    })
}

fn out_dir() -> PathBuf {
    match env::args().nth(1) {
        Some(dir) => PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/tray-sheets"),
    }
}

fn main() -> io::Result<()> {
    let dir = out_dir();
    fs::create_dir_all(&dir)?;
    let entries = cases()
        .iter()
        .enumerate()
        .map(|(i, case)| write_case(&dir, i, case))
        .collect::<io::Result<Vec<_>>>()?;
    write_png(&dir, "sheet-light", &sheet::build(&LIGHT, &entries))?;
    write_png(&dir, "sheet-dark", &sheet::build(&DARK, &entries))?;
    println!("written to {}", dir.display());
    Ok(())
}
