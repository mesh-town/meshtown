//! RGBA raster (straight, not premultiplied alpha) and the icon sheet operations.
//!
//! What it does: stores the raster; scales it up by an integer factor with nearest neighbor (no smoothing),
//! centers it on a transparent canvas, overlays it on an opaque background by alpha, draws 3×5
//! digits for case numbers and fills rectangles.
//! What it doesn't do: it doesn't draw the icon (that is `ring_rgba_lit` from the branch code) and doesn't write files.
//! Links: `sheet.rs` builds the sheets from this; `png.rs` encodes `Image`.

/// A `w×h` raster, RGBA bytes row by row from top to bottom.
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

/// Digits 0-9, a 3×5 glyph: one byte per row, the low three bits are pixels from left to right.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

impl Image {
    /// A raster filled with a single color.
    pub fn filled(w: u32, h: u32, rgba: [u8; 4]) -> Image {
        let px = rgba.iter().copied().cycle().take((w * h * 4) as usize);
        Image {
            w,
            h,
            px: px.collect(),
        }
    }

    /// A `side×side` square from ready-made RGBA bytes.
    pub fn square(side: u32, px: Vec<u8>) -> Image {
        Image {
            w: side,
            h: side,
            px,
        }
    }

    fn index(&self, x: u32, y: u32) -> usize {
        ((y * self.w + x) * 4) as usize
    }

    fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let i = self.index(x, y);
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    fn set(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        let i = self.index(x, y);
        self.px[i..i + 4].copy_from_slice(&rgba);
    }

    /// Scales up by `k` times with nearest neighbor: each pixel becomes a `k×k` square.
    pub fn scaled(&self, k: u32) -> Image {
        let mut out = Image::filled(self.w * k, self.h * k, [0; 4]);
        for y in 0..out.h {
            for x in 0..out.w {
                out.set(x, y, self.get(x / k, y / k));
            }
        }
        out
    }

    /// A transparent `side×side` canvas with the raster in the center (the extra pixel goes to the right and bottom).
    pub fn centered(&self, side: u32) -> Image {
        let mut out = Image::filled(side, side, [0; 4]);
        out.blit(self, (side - self.w) / 2, (side - self.h) / 2);
        out
    }

    /// Copies `src` to the point `(x0, y0)` as is, alpha included.
    pub fn blit(&mut self, src: &Image, x0: u32, y0: u32) {
        for y in 0..src.h {
            for x in 0..src.w {
                self.set(x0 + x, y0 + y, src.get(x, y));
            }
        }
    }

    /// Overlays `src` at the point `(x0, y0)` on an opaque background by the alpha of `src`.
    pub fn over(&mut self, src: &Image, x0: u32, y0: u32) {
        for y in 0..src.h {
            for x in 0..src.w {
                let px = mix(src.get(x, y), self.get(x0 + x, y0 + y));
                self.set(x0 + x, y0 + y, px);
            }
        }
    }

    /// Fills a rectangle with a single color.
    pub fn fill_rect(&mut self, x0: u32, y0: u32, w: u32, h: u32, rgba: [u8; 4]) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                self.set(x, y, rgba);
            }
        }
    }

    /// Draws a number in 3×5 digits scaled up by `k` times; `(x0, y0)` is the top left of the first digit.
    pub fn number(&mut self, n: usize, x0: u32, y0: u32, k: u32, rgba: [u8; 4]) {
        for (i, ch) in n.to_string().bytes().enumerate() {
            let glyph = DIGITS[usize::from(ch - b'0')];
            self.glyph(glyph, x0 + i as u32 * 4 * k, y0, k, rgba);
        }
    }

    fn glyph(&mut self, glyph: [u8; 5], x0: u32, y0: u32, k: u32, rgba: [u8; 4]) {
        for (row, bits) in glyph.iter().enumerate() {
            for col in (0..3u32).filter(|&col| bits >> (2 - col) & 1 == 1) {
                self.fill_rect(x0 + col * k, y0 + row as u32 * k, k, k, rgba);
            }
        }
    }
}

/// Pixel `top` over the opaque `bottom`: `top·a + bottom·(1 − a)`, the result alpha is 255.
fn mix(top: [u8; 4], bottom: [u8; 4]) -> [u8; 4] {
    let a = u32::from(top[3]);
    let c = |i: usize| ((u32::from(top[i]) * a + u32::from(bottom[i]) * (255 - a) + 127) / 255) as u8;
    [c(0), c(1), c(2), 255]
}
