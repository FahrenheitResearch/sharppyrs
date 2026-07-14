//! Profile interpolation (port of `sharppy.sharptab.interp`).
//!
//! Fields are interpolated linearly against log10(pressure), heights against
//! height. Out-of-range queries return NaN (numpy's masked), while queries on
//! the boundary return the boundary value.

use crate::profile::Profile;
use crate::utils::{comp2vec, qc};

/// np.interp equivalent over the finite pairs of (xs, ys); xs must be
/// ascending. Out of range -> NaN (mirroring `left=masked, right=masked`),
/// exact boundary -> boundary value.
fn interp1(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    if x.is_nan() || xs.is_empty() {
        return f64::NAN;
    }
    let x0 = xs[0];
    let xn = xs[xs.len() - 1];
    // Boundary fix mirroring the np.isclose() endpoint handling.
    if close(x, x0) {
        return ys[0];
    }
    if close(x, xn) {
        return ys[ys.len() - 1];
    }
    if x < x0 || x > xn {
        return f64::NAN;
    }
    // Binary search for the bracketing interval.
    let mut lo = 0usize;
    let mut hi = xs.len() - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if xs[mid] <= x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (xa, xb) = (xs[lo], xs[hi]);
    let (ya, yb) = (ys[lo], ys[hi]);
    if xb == xa {
        return ya;
    }
    ya + (x - xa) / (xb - xa) * (yb - ya)
}

#[inline]
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-8 + 1e-5 * b.abs()
}

/// Collect the finite (coordinate, field) pairs in ascending coordinate order.
/// `reverse` mirrors the `[::-1]` the Python code applies to pressure axes.
fn finite_pairs(coord: &[f64], field: &[f64], reverse: bool) -> (Vec<f64>, Vec<f64>) {
    let mut xs = Vec::with_capacity(coord.len());
    let mut ys = Vec::with_capacity(coord.len());
    let iter: Box<dyn Iterator<Item = usize>> = if reverse {
        Box::new((0..coord.len()).rev())
    } else {
        Box::new(0..coord.len())
    };
    for i in iter {
        if coord[i].is_finite() && field[i].is_finite() {
            xs.push(coord[i]);
            ys.push(field[i]);
        }
    }
    (xs, ys)
}

fn interp_pres_field(prof: &Profile, p: f64, field: &[f64]) -> f64 {
    let (xs, ys) = finite_pairs(&prof.logp, field, true);
    interp1(p.log10(), &xs, &ys)
}

/// Pressure (hPa) at height `h` (m MSL).
pub fn pres(prof: &Profile, h: f64) -> f64 {
    let (xs, ys) = finite_pairs(&prof.hght, &prof.logp, false);
    let v = interp1(h, &xs, &ys);
    10f64.powf(v)
}

/// Height (m MSL) at pressure `p` (hPa).
pub fn hght(prof: &Profile, p: f64) -> f64 {
    interp_pres_field(prof, p, &prof.hght)
}

/// Temperature (C) at pressure `p`.
pub fn temp(prof: &Profile, p: f64) -> f64 {
    interp_pres_field(prof, p, &prof.tmpc)
}

/// Dewpoint (C) at pressure `p`.
pub fn dwpt(prof: &Profile, p: f64) -> f64 {
    interp_pres_field(prof, p, &prof.dwpc)
}

/// Virtual temperature (C) at pressure `p`.
pub fn vtmp(prof: &Profile, p: f64) -> f64 {
    interp_pres_field(prof, p, &prof.vtmp)
}

/// Theta-E (C) at pressure `p`.
pub fn thetae(prof: &Profile, p: f64) -> f64 {
    interp_pres_field(prof, p, &prof.thetae)
}

/// Omega (Pa/s) at pressure `p`.
pub fn omeg(prof: &Profile, p: f64) -> f64 {
    if prof.omeg.is_empty() {
        return f64::NAN;
    }
    interp_pres_field(prof, p, &prof.omeg)
}

/// U/V wind components (kts) at pressure `p`.
pub fn components(prof: &Profile, p: f64) -> (f64, f64) {
    if !prof.wdir.iter().any(|w| qc(*w)) {
        return (f64::NAN, f64::NAN);
    }
    (
        interp_pres_field(prof, p, &prof.u),
        interp_pres_field(prof, p, &prof.v),
    )
}

/// Wind direction/speed (deg, kts) at pressure `p`.
pub fn vec(prof: &Profile, p: f64) -> (f64, f64) {
    let (u, v) = components(prof, p);
    if !qc(u) || !qc(v) {
        return (f64::NAN, f64::NAN);
    }
    comp2vec(u, v)
}

/// Convert m MSL to m AGL.
pub fn to_agl(prof: &Profile, h: f64) -> f64 {
    h - prof.hght[prof.sfc]
}

/// Convert m AGL to m MSL.
pub fn to_msl(prof: &Profile, h: f64) -> f64 {
    h + prof.hght[prof.sfc]
}
