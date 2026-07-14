//! Thermodynamic library (faithful port of `sharppy.sharptab.thermo`).
//!
//! All temperatures are Celsius and pressures hPa unless noted. NaN stands in
//! for numpy's masked values and propagates through every routine.

use crate::constants::{ROCP, ZEROCNK};

// Constants used by temp_at_mixrat.
const C1: f64 = 0.0498646455;
const C2: f64 = 2.4082965;
const C3: f64 = 7.07475;
const C4: f64 = 38.9114;
const C5: f64 = 0.0915;
const C6: f64 = 1.2035;
const EPS: f64 = 0.62197;

/// Lift a parcel to its LCL; returns `(lcl_pres_hpa, lcl_temp_c)`.
pub fn drylift(p: f64, t: f64, td: f64) -> (f64, f64) {
    let t2 = lcltemp(t, td);
    let p2 = thalvl(theta(p, t, 1000.0), t2);
    (p2, t2)
}

/// Temperature (C) of a parcel raised to its LCL.
pub fn lcltemp(t: f64, td: f64) -> f64 {
    let s = t - td;
    let dlt = s * (1.2185 + 0.001278 * t + s * (-0.00219 + 1.173e-5 * s - 0.0000052 * t));
    t - dlt
}

/// Pressure level (hPa) at which a parcel of the given potential temperature
/// has the given temperature.
pub fn thalvl(theta: f64, t: f64) -> f64 {
    let t = t + ZEROCNK;
    let theta = theta + ZEROCNK;
    1000.0 / (theta / t).powf(1.0 / ROCP)
}

/// Potential temperature (C) referenced to `p2` (default 1000 hPa).
pub fn theta(p: f64, t: f64, p2: f64) -> f64 {
    (t + ZEROCNK) * (p2 / p).powf(ROCP) - ZEROCNK
}

/// Equivalent potential temperature (C).
pub fn thetae(p: f64, t: f64, td: f64) -> f64 {
    let (p2, t2) = drylift(p, t, td);
    theta(100.0, wetlift(p2, t2, 100.0), 1000.0)
}

/// Virtual temperature (C); falls back to `t` when `td` is missing.
pub fn virtemp(p: f64, t: f64, td: f64) -> f64 {
    let tk = t + ZEROCNK;
    let w = 0.001 * mixratio(p, td);
    let vt = (tk * (1.0 + w / EPS) / (1.0 + w)) - ZEROCNK;
    if !crate::utils::qc(vt) {
        t
    } else {
        vt
    }
}

/// Wobus function: correction to theta for saturated potential temperature.
pub fn wobf(t: f64) -> f64 {
    let t = t - 20.0;
    if t.is_nan() {
        return f64::NAN;
    }
    if t <= 0.0 {
        let npol = 1.0
            + t * (-8.841660499999999e-3
                + t * (1.4714143e-4
                    + t * (-9.671989000000001e-7 + t * (-3.2607217e-8 + t * (-3.8598073e-10)))));
        15.13 / npol.powi(4)
    } else {
        let ppol = t
            * (4.9618922e-07
                + t * (-6.1059365e-09
                    + t * (3.9401551e-11 + t * (-1.2588129e-13 + t * (1.6688280e-16)))));
        let ppol = 1.0 + t * (3.6182989e-03 + t * (-1.3603273e-05 + ppol));
        (29.93 / ppol.powi(4)) + (0.96 * t) - 14.8
    }
}

/// Temperature (C) of a saturated parcel `thetam` lifted to pressure `p`.
pub fn satlift(p: f64, thetam: f64) -> f64 {
    satlift_conv(p, thetam, 0.1)
}

/// `satlift` with an explicit convergence criterion (C).
pub fn satlift_conv(p: f64, thetam: f64, conv: f64) -> f64 {
    if p.is_nan() || thetam.is_nan() {
        return f64::NAN;
    }
    if (p - 1000.0).abs() - 0.001 <= 0.0 {
        return thetam;
    }
    let pwrp = (p / 1000.0).powf(ROCP);
    let mut t1 = (thetam + ZEROCNK) * pwrp - ZEROCNK;
    let mut e1 = wobf(t1) - wobf(thetam);
    let mut rate = 1.0;
    let mut t2;
    let mut e2;
    let mut eor;
    // First pass, then successive secant iterations (same as the original loop).
    loop {
        t2 = t1 - e1 * rate;
        e2 = (t2 + ZEROCNK) / pwrp - ZEROCNK;
        e2 += wobf(t2) - wobf(e2) - thetam;
        eor = e2 * rate;
        if eor.is_nan() {
            return f64::NAN;
        }
        if eor.abs() - conv <= 0.0 {
            break;
        }
        rate = (t2 - t1) / (e2 - e1);
        t1 = t2;
        e1 = e2;
    }
    t2 - eor
}

/// Lift a parcel moist adiabatically from `(p, t)` to level `p2`.
pub fn wetlift(p: f64, t: f64, p2: f64) -> f64 {
    let thta = theta(p, t, 1000.0);
    if thta.is_nan() || p2.is_nan() {
        return f64::NAN;
    }
    let thetam = thta - wobf(thta) + wobf(t);
    satlift(p2, thetam)
}

/// Temperature (C) of a parcel `(p, t, td)` lifted to pressure `lev`.
pub fn lifted(p: f64, t: f64, td: f64, lev: f64) -> f64 {
    let (p2, t2) = drylift(p, t, td);
    wetlift(p2, t2, lev)
}

/// Saturation vapor pressure (hPa) over liquid at temperature `t` (C).
pub fn vappres(t: f64) -> f64 {
    let mut pol = t * (1.1112018e-17 + t * -3.0994571e-20);
    pol = t * (2.1874425e-13 + t * (-1.789232e-15 + pol));
    pol = t * (4.3884180e-09 + t * (-2.988388e-11 + pol));
    pol = t * (7.8736169e-05 + t * (-6.111796e-07 + pol));
    pol = 0.99999683 + t * (-9.082695e-03 + pol);
    6.1078 / pol.powi(8)
}

/// Mixing ratio (g/kg) of a parcel.
pub fn mixratio(p: f64, t: f64) -> f64 {
    let x = 0.02 * (t - 12.5 + 7500.0 / p);
    let wfw = 1.0 + 0.0000045 * p + 0.0014 * x * x;
    let fwesw = wfw * vappres(t);
    621.97 * (fwesw / (p - fwesw))
}

/// Temperature (C) of air at the given mixing ratio (g/kg) and pressure (hPa).
pub fn temp_at_mixrat(w: f64, p: f64) -> f64 {
    let x = (w * p / (622.0 + w)).log10();
    10f64.powf(C1 * x + C2) - C3 + C4 * (10f64.powf(C5 * x) - C6).powi(2) - ZEROCNK
}

/// Wetbulb temperature (C) of the given parcel.
pub fn wetbulb(p: f64, t: f64, td: f64) -> f64 {
    let (p2, t2) = drylift(p, t, td);
    wetlift(p2, t2, p)
}

/// Celsius to Fahrenheit.
#[inline]
pub fn ctof(t: f64) -> f64 {
    1.8 * t + 32.0
}

/// Celsius to Kelvin.
#[inline]
pub fn ctok(t: f64) -> f64 {
    t + ZEROCNK
}

/// Kelvin to Celsius.
#[inline]
pub fn ktoc(t: f64) -> f64 {
    t - ZEROCNK
}
