//! Writing PNG without third-party libraries: RGBA 8 bit, no compression (raw deflate blocks).
//!
//! What it does: `encode` turns `Image` into PNG bytes: the signature, IHDR, one IDAT of an
//! uncompressed zlib stream, IEND. The files are larger than compressed ones, but there is no dependency: `png` is in the project
//! only transitively through `tray-icon`, and the example can't reach it.
//! What it doesn't do: it doesn't compress, doesn't read PNG, doesn't write to disk.
//! Links: `main.rs` calls `encode` and writes the result to a file.

use crate::image::Image;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The largest length of one uncompressed deflate block.
const STORED_MAX: usize = 65535;

/// A PNG file from a raster.
pub fn encode(img: &Image) -> Vec<u8> {
    let mut out = SIGNATURE.to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&img.w.to_be_bytes());
    ihdr.extend_from_slice(&img.h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, *b"IHDR", &ihdr);
    chunk(&mut out, *b"IDAT", &zlib_stored(&scanlines(img)));
    chunk(&mut out, *b"IEND", &[]);
    out
}

/// Raster rows, each with filter byte 0 at the start.
fn scanlines(img: &Image) -> Vec<u8> {
    let row = (img.w * 4) as usize;
    img.px
        .chunks(row)
        .flat_map(|line| std::iter::once(0u8).chain(line.iter().copied()))
        .collect()
}

/// A zlib stream of uncompressed blocks and an Adler-32 sum.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = data.chunks(STORED_MAX).collect();
    for (i, block) in blocks.iter().enumerate() {
        let len = block.len() as u16;
        out.push(u8::from(i + 1 == blocks.len()));
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (a, b) = data.iter().fold((1u32, 0u32), |(a, b), &byte| {
        let a = (a + u32::from(byte)) % 65521;
        (a, (b + a) % 65521)
    });
    (b << 16) | a
}

/// A PNG chunk: length, type, data, CRC-32 of the type and data.
fn chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let table = crc_table();
    let crc = data.iter().fold(u32::MAX, |crc, &byte| {
        table[((crc ^ u32::from(byte)) & 0xFF) as usize] ^ (crc >> 8)
    });
    !crc
}

fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (n, slot) in table.iter_mut().enumerate() {
        *slot = (0..8).fold(n as u32, |c, _| {
            (c >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(c & 1))
        });
    }
    table
}
