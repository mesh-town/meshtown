//! C23 `fmaximum_num` / `fminimum_num` for soft-float ARM musl.
//!
//! What it does: on `armv7-unknown-linux-musleabi` rustc lowers `f64::max` / `f64::min` (and the `f32` ones) to
//! libcalls of the C23 functions `fmaximum_num`, `fminimum_num`, `fmaximum_numf`, `fminimum_numf`; the musl bundled
//! with zig does not have them, so the link fails. This module defines them with C23 semantics: a NaN argument
//! yields the other one, `-0.0` is below `+0.0`.
//! What it doesn't do: it is not built for other targets (there libc or the instruction set provides them); in
//! tests the plain functions are checked on the host.
//! Links: declared from `lib.rs`; tests — `libm_shim_tests.rs`.

pub fn maximum_num(x: f64, y: f64) -> f64 {
    if x.is_nan() || y > x || (y == x && y.is_sign_positive()) {
        return y;
    }
    x
}

pub fn minimum_num(x: f64, y: f64) -> f64 {
    if x.is_nan() || y < x || (y == x && y.is_sign_negative()) {
        return y;
    }
    x
}

pub fn maximum_numf(x: f32, y: f32) -> f32 {
    if x.is_nan() || y > x || (y == x && y.is_sign_positive()) {
        return y;
    }
    x
}

pub fn minimum_numf(x: f32, y: f32) -> f32 {
    if x.is_nan() || y < x || (y == x && y.is_sign_negative()) {
        return y;
    }
    x
}

#[cfg(all(target_arch = "arm", target_env = "musl"))]
mod exported {
    #[no_mangle]
    pub extern "C" fn fmaximum_num(x: f64, y: f64) -> f64 {
        super::maximum_num(x, y)
    }

    #[no_mangle]
    pub extern "C" fn fminimum_num(x: f64, y: f64) -> f64 {
        super::minimum_num(x, y)
    }

    #[no_mangle]
    pub extern "C" fn fmaximum_numf(x: f32, y: f32) -> f32 {
        super::maximum_numf(x, y)
    }

    #[no_mangle]
    pub extern "C" fn fminimum_numf(x: f32, y: f32) -> f32 {
        super::minimum_numf(x, y)
    }
}

#[cfg(test)]
#[path = "libm_shim_tests.rs"]
mod tests;
