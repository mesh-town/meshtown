//! A contact sheet of icons on a "light/dark menu bar" backdrop.
//!
//! What it does: assembles one raster: on top a strip "as in the menu bar" (all icons at natural
//! size, 36 px and 22 px), below a grid of cases, 3 per row; in a cell the 36 px icon is on the left and the
//! 22 px on the right, both an integer factor larger with nearest neighbor in 256×256 slots, with the case number in the corner.
//! Icons are laid on the backdrop by alpha, as the menu bar does it.
//! What it doesn't do: it doesn't draw icons and doesn't write files; there are no captions except the number.
//! Links: `main.rs` supplies the icon rasters and the backdrop colors; the order of cells is the order of cases.

use crate::image::Image;

/// The side of the slot for an enlarged icon.
pub const SLOT: u32 = 256;
const PAD: u32 = 16;
const COLS: u32 = 3;
const LABEL_H: u32 = 28;
const LABEL_SCALE: u32 = 4;
/// The step of icons in the natural-size strip.
const STRIP_STEP: u32 = 48;
const STRIP_H: u32 = 88;

/// Sheet colors: the menu bar backdrop, the margin between cells and the color of numbers.
pub struct Theme {
    pub bar: [u8; 4],
    pub gutter: [u8; 4],
    pub ink: [u8; 4],
}

/// Icons of one case: 36 px and 22 px as `ring_rgba_lit` returned them, and the same enlarged.
pub struct Entry {
    pub small: [Image; 2],
    pub zoomed: [Image; 2],
}

/// A sheet of all cases in order on the `theme` backdrop.
pub fn build(theme: &Theme, entries: &[Entry]) -> Image {
    let cell_w = 2 * SLOT;
    let width = COLS * cell_w + (COLS + 1) * PAD;
    let rows = (entries.len() as u32).div_ceil(COLS);
    let grid_top = 2 * PAD + STRIP_H;
    let height = grid_top + rows * (LABEL_H + SLOT + PAD);
    let mut sheet = Image::filled(width, height, theme.gutter);
    sheet.fill_rect(PAD, PAD, width - 2 * PAD, STRIP_H, theme.bar);
    strip(&mut sheet, entries);
    for (i, entry) in entries.iter().enumerate() {
        let (col, row) = (i as u32 % COLS, i as u32 / COLS);
        let x = PAD + col * (cell_w + PAD);
        let y = grid_top + row * (LABEL_H + SLOT + PAD);
        cell(&mut sheet, theme, (x, y), (i + 1, entry));
    }
    sheet
}

/// A "as in the menu bar" strip: 36 px icons in the first row, 22 px in the second, no enlargement.
fn strip(sheet: &mut Image, entries: &[Entry]) {
    for (i, entry) in entries.iter().enumerate() {
        let x = 2 * PAD + i as u32 * STRIP_STEP;
        sheet.over(&entry.small[0], x, PAD + 10);
        sheet.over(&entry.small[1], x + 7, PAD + 10 + 36 + 10);
    }
}

/// A case cell: the number and two enlarged icons on a menu bar backdrop.
fn cell(sheet: &mut Image, theme: &Theme, (x, y): (u32, u32), (number, entry): (usize, &Entry)) {
    sheet.fill_rect(x, y, 2 * SLOT, LABEL_H + SLOT, theme.bar);
    sheet.number(number, x + 8, y + 4, LABEL_SCALE, theme.ink);
    sheet.over(&entry.zoomed[0], x, y + LABEL_H);
    sheet.over(&entry.zoomed[1], x + SLOT, y + LABEL_H);
}
