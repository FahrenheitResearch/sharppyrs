//! Kinematic routines (port of `sharppy.sharptab.winds`).

use crate::interp;
use crate::profile::Profile;
use crate::utils::{kts2ms, mag, ms2kts, qc};

/// Pressure levels from `pbot` down to `ptop` (inclusive) in -1 hPa steps,
/// mirroring `np.arange(pbot, ptop + dp, dp)` with dp = -1.
fn pres_steps(pbot: f64, ptop: f64) -> Vec<f64> {
    let mut ps = Vec::new();
    let mut p = pbot;
    while p > ptop - 1.0 {
        ps.push(p);
        p -= 1.0;
    }
    ps
}

/// Pressure-weighted mean wind through a layer (kts).
pub fn mean_wind(prof: &Profile, pbot: f64, ptop: f64, stu: f64, stv: f64) -> (f64, f64) {
    if !qc(pbot) || !qc(ptop) {
        return (f64::NAN, f64::NAN);
    }
    if !prof.wdir.iter().any(|w| qc(*w)) {
        return (f64::NAN, f64::NAN);
    }
    let ps = pres_steps(pbot, ptop);
    let mut usum = 0.0;
    let mut vsum = 0.0;
    let mut wsum = 0.0;
    for &p in &ps {
        let (u, v) = interp::components(prof, p);
        if qc(u) && qc(v) {
            usum += u * p;
            vsum += v * p;
            wsum += p;
        }
    }
    if wsum == 0.0 {
        return (f64::NAN, f64::NAN);
    }
    (usum / wsum - stu, vsum / wsum - stv)
}

/// Non-pressure-weighted mean wind through a layer (kts).
pub fn mean_wind_npw(prof: &Profile, pbot: f64, ptop: f64, stu: f64, stv: f64) -> (f64, f64) {
    if !qc(pbot) || !qc(ptop) {
        return (f64::NAN, f64::NAN);
    }
    let ps = pres_steps(pbot, ptop);
    let mut usum = 0.0;
    let mut vsum = 0.0;
    let mut count = 0.0;
    for &p in &ps {
        let (u, v) = interp::components(prof, p);
        if qc(u) && qc(v) {
            usum += u;
            vsum += v;
            count += 1.0;
        }
    }
    if count == 0.0 {
        return (f64::NAN, f64::NAN);
    }
    (usum / count - stu, vsum / count - stv)
}

/// Wind shear between `pbot` and `ptop` (kts).
pub fn wind_shear(prof: &Profile, pbot: f64, ptop: f64) -> (f64, f64) {
    if !prof.wdir.iter().any(|w| qc(*w)) || !qc(pbot) || !qc(ptop) {
        return (f64::NAN, f64::NAN);
    }
    let (ubot, vbot) = interp::components(prof, pbot);
    let (utop, vtop) = interp::components(prof, ptop);
    (utop - ubot, vtop - vbot)
}

/// Bunkers storm motion without a parcel (SFC-6km mean wind / shear).
/// Returns `(rstu, rstv, lstu, lstv)` in kts.
pub fn non_parcel_bunkers_motion(prof: &Profile) -> (f64, f64, f64, f64) {
    if !prof.wdir.iter().any(|w| qc(*w)) {
        return (f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    }
    let d = ms2kts(7.5);
    let msl6km = interp::to_msl(prof, 6000.0);
    let p6km = interp::pres(prof, msl6km);
    let (mnu6, mnv6) = mean_wind_npw(prof, prof.pres[prof.sfc], p6km, 0.0, 0.0);
    let (shru, shrv) = wind_shear(prof, prof.pres[prof.sfc], p6km);
    let tmp = d / mag(shru, shrv);
    (
        mnu6 + tmp * shrv,
        mnv6 - tmp * shru,
        mnu6 - tmp * shrv,
        mnv6 + tmp * shru,
    )
}

/// Storm-relative helicity of the layer `lower..upper` (m AGL) for storm
/// motion `(stu, stv)` kts. Returns `(total, positive, negative)` in m2/s2.
/// Uses the `exact = True` path of the original.
pub fn helicity(prof: &Profile, lower: f64, upper: f64, stu: f64, stv: f64) -> (f64, f64, f64) {
    if !prof.wdir.iter().any(|w| qc(*w)) || !qc(lower) || !qc(upper) || !qc(stu) || !qc(stv) {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    if lower == upper {
        return (0.0, 0.0, 0.0);
    }
    let lower = interp::to_msl(prof, lower);
    let upper = interp::to_msl(prof, upper);
    let plower = interp::pres(prof, lower);
    let pupper = interp::pres(prof, upper);
    if !qc(plower) || !qc(pupper) {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    // Exact path: boundary-interpolated winds plus every valid level between.
    let mut us: Vec<f64> = Vec::new();
    let mut vs: Vec<f64> = Vec::new();
    let (u1, v1) = interp::components(prof, plower);
    us.push(u1);
    vs.push(v1);
    // ind1: first index with pres <= plower; ind2: last index with pres >= pupper.
    let mut ind1 = None;
    let mut ind2 = None;
    for i in 0..prof.pres.len() {
        if prof.pres[i].is_finite() {
            if ind1.is_none() && plower >= prof.pres[i] {
                ind1 = Some(i);
            }
            if pupper <= prof.pres[i] {
                ind2 = Some(i);
            }
        }
    }
    if let (Some(i1), Some(i2)) = (ind1, ind2) {
        for i in i1..=i2 {
            if qc(prof.u[i]) && qc(prof.v[i]) {
                us.push(prof.u[i]);
                vs.push(prof.v[i]);
            }
        }
    }
    let (u2, v2) = interp::components(prof, pupper);
    us.push(u2);
    vs.push(v2);

    let mut phel = 0.0;
    let mut nhel = 0.0;
    for k in 0..us.len() - 1 {
        let sru0 = kts2ms(us[k] - stu);
        let srv0 = kts2ms(vs[k] - stv);
        let sru1 = kts2ms(us[k + 1] - stu);
        let srv1 = kts2ms(vs[k + 1] - stv);
        let layer = sru1 * srv0 - sru0 * srv1;
        if layer.is_nan() {
            continue;
        }
        if layer > 0.0 {
            phel += layer;
        } else if layer < 0.0 {
            nhel += layer;
        }
    }
    (phel + nhel, phel, nhel)
}
