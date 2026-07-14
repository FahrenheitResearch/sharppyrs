//! Small helpers (port of `sharppy.sharptab.utils`). NaN plays the role of
//! numpy's masked values throughout the crate.

use crate::constants::TOL;

/// Quality control: is this a usable value? (port of `utils.QC`)
#[inline]
pub fn qc(val: f64) -> bool {
    val.is_finite() && val > -9990.0
}

/// Convert meters per second to knots.
#[inline]
pub fn ms2kts(val: f64) -> f64 {
    val * 1.94384449
}

/// Convert knots to meters per second.
#[inline]
pub fn kts2ms(val: f64) -> f64 {
    val * 0.514444
}

/// Convert meters to feet.
#[inline]
pub fn m2ft(val: f64) -> f64 {
    val * 3.2808399
}

/// Wind direction/speed to U/V components (kts in, kts out).
#[inline]
pub fn vec2comp(wdir: f64, wspd: f64) -> (f64, f64) {
    let rad = wdir.to_radians();
    let u = wspd * -rad.sin();
    let v = wspd * -rad.cos();
    (u, v)
}

/// U/V components to wind direction/speed.
pub fn comp2vec(u: f64, v: f64) -> (f64, f64) {
    let wspd = mag(u, v);
    let mut wdir = (-u).atan2(-v).to_degrees();
    if wdir < 0.0 {
        wdir += 360.0;
    }
    if wspd.abs() < TOL {
        wdir = 0.0;
    }
    (wdir, wspd)
}

/// Vector magnitude.
#[inline]
pub fn mag(u: f64, v: f64) -> f64 {
    (u * u + v * v).sqrt()
}

/// Format a value as a rounded integer string, `--` when missing
/// (port of `utils.INT2STR`).
pub fn int2str(val: f64) -> String {
    if !qc(val) {
        return "--".to_string();
    }
    format!("{}", val.round() as i64)
}

/// Format a value with the given precision, `--` when missing
/// (port of `utils.FLOAT2STR`).
pub fn float2str(val: f64, precision: usize) -> String {
    if !qc(val) {
        return "--".to_string();
    }
    format!("{val:.precision$}")
}
