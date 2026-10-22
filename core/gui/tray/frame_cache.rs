//! A cache of tray icons for the spinner frames: the raster of a ring frame is built once per ring and reused
//! on every later turn of the circle.
//!
//! What it does: `FrameCache::get_or_build` keeps built values by `IconPicture` key, only for pictures with a
//! highlighted sector, and forgets everything when the ring changes. What it does not do: it builds nothing
//! itself (the caller passes the builder) and knows nothing about the tray.

use crate::app::view_picture::IconPicture;
use crate::app::view_ring::Ring;

/// Built frames by picture; `ring` is the ring they were built for.
pub(super) struct FrameCache<T> {
    ring: Option<Ring>,
    frames: Vec<(IconPicture, T)>,
}

impl<T: Clone> FrameCache<T> {
    pub(super) fn new() -> Self {
        FrameCache { ring: None, frames: Vec::new() }
    }

    /// The value for `key`: a picture without a highlight is built without caching; a new ring clears the cache;
    /// a missing frame is built and, on success, stored.
    pub(super) fn get_or_build(
        &mut self,
        key: IconPicture,
        build: impl FnOnce(IconPicture) -> Result<T, String>,
    ) -> Result<T, String> {
        if key.lit.is_none() {
            return build(key);
        }
        if key.ring != self.ring {
            self.frames.clear();
            self.ring = key.ring;
        }
        if let Some((_, value)) = self.frames.iter().find(|(k, _)| *k == key) {
            return Ok(value.clone());
        }
        let value = build(key)?;
        self.frames.push((key, value.clone()));
        Ok(value)
    }
}

#[cfg(test)]
#[path = "frame_cache_tests.rs"]
mod tests;
