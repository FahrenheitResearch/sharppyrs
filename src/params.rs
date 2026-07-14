//! Convective parameters (port of the `sharppy.sharptab.params` routines the
//! skew-T needs: parcel definition and lifting, effective inflow layer,
//! Bunkers storm motion, DCAPE, lapse rates, temperature levels).

use crate::constants::{G, TOL};
use crate::interp;
use crate::profile::Profile;
use crate::thermo;
use crate::utils::{mag, ms2kts, qc};
use crate::winds;

/// Which lifted parcel to display (subset of `DefineParcel` flags).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ParcelType {
    /// Observed surface parcel (flag 1).
    Surface,
    /// Forecast surface parcel (flag 2).
    Forecast,
    /// Most unstable parcel in the lowest 300 hPa (flag 3).
    #[default]
    MostUnstable,
    /// 100-hPa mean mixed layer parcel (flag 4).
    MixedLayer,
}

/// Lifted parcel result (subset of the SHARPpy `Parcel` object used by the
/// skew-T display). NaN = not available.
#[derive(Clone, Debug)]
pub struct Parcel {
    /// Lifting parcel level pressure (hPa).
    pub lpl_pres: f64,
    /// Lifting parcel temperature (C).
    pub lpl_tmpc: f64,
    /// Lifting parcel dewpoint (C).
    pub lpl_dwpc: f64,
    /// LCL pressure (hPa) / height (m AGL).
    pub lclpres: f64,
    pub lclhght: f64,
    /// LFC pressure (hPa) / height (m AGL).
    pub lfcpres: f64,
    pub lfchght: f64,
    /// EL pressure (hPa) / height (m AGL).
    pub elpres: f64,
    pub elhght: f64,
    /// Maximum parcel level (hPa).
    pub mplpres: f64,
    /// CAPE (J/kg).
    pub bplus: f64,
    /// CIN below 500 hPa (J/kg).
    pub bminus: f64,
    /// Pressures (hPa) of the 0 / -10 / -20 / -30 C environment levels.
    pub p0c: f64,
    pub pm10c: f64,
    pub pm20c: f64,
    pub pm30c: f64,
    /// Heights (m MSL) of the 0 / -20 / -30 C environment levels.
    pub hght0c: f64,
    pub hghtm20c: f64,
    pub hghtm30c: f64,
    /// Parcel virtual-temperature trace (C) and its pressures (hPa).
    pub ttrace: Vec<f64>,
    pub ptrace: Vec<f64>,
}

impl Default for Parcel {
    fn default() -> Self {
        Parcel {
            lpl_pres: f64::NAN,
            lpl_tmpc: f64::NAN,
            lpl_dwpc: f64::NAN,
            lclpres: f64::NAN,
            lclhght: f64::NAN,
            lfcpres: f64::NAN,
            lfchght: f64::NAN,
            elpres: f64::NAN,
            elhght: f64::NAN,
            mplpres: f64::NAN,
            bplus: f64::NAN,
            bminus: f64::NAN,
            p0c: f64::NAN,
            pm10c: f64::NAN,
            pm20c: f64::NAN,
            pm30c: f64::NAN,
            hght0c: f64::NAN,
            hghtm20c: f64::NAN,
            hghtm30c: f64::NAN,
            ttrace: Vec::new(),
            ptrace: Vec::new(),
        }
    }
}

/// `np.arange(start, stop, -1.0)`: descending values strictly greater than `stop`.
fn arange_desc(start: f64, stop: f64) -> Vec<f64> {
    let mut v = Vec::new();
    if !start.is_finite() || !stop.is_finite() {
        return v;
    }
    let mut p = start;
    while p > stop {
        v.push(p);
        p -= 1.0;
    }
    v
}

/// Level (hPa) of the first occurrence of `temp` (C) in the temperature
/// profile (port of `params.temp_lvl`).
pub fn temp_lvl(prof: &Profile, temp: f64) -> f64 {
    // Gather finite (t, logp) pairs.
    let mut ts = Vec::new();
    let mut lps = Vec::new();
    let mut ps = Vec::new();
    for i in 0..prof.pres.len() {
        if prof.tmpc[i].is_finite() && prof.logp[i].is_finite() {
            ts.push(prof.tmpc[i]);
            lps.push(prof.logp[i]);
            ps.push(prof.pres[i]);
        }
    }
    if ts.is_empty() {
        return f64::NAN;
    }
    let any_le = ts.iter().any(|t| *t - temp <= 0.0);
    let any_ge = ts.iter().any(|t| *t - temp >= 0.0);
    if !any_le || !any_ge {
        return f64::NAN;
    }
    if let Some(i) = ts.iter().position(|t| *t - temp == 0.0) {
        return ps[i];
    }
    for i in 0..ts.len() - 1 {
        let d0 = ts[i] - temp;
        let d1 = ts[i + 1] - temp;
        if d0 * d1 < 0.0 {
            // np.interp(temp, [t[i+1], t[i]], [logp[i+1], logp[i]])
            let (t_lo, t_hi) = (ts[i + 1], ts[i]);
            let (lp_lo, lp_hi) = (lps[i + 1], lps[i]);
            let lp = if t_hi == t_lo {
                lp_lo
            } else if temp <= t_lo.min(t_hi) {
                lp_lo
            } else if temp >= t_lo.max(t_hi) {
                lp_hi
            } else {
                lp_lo + (temp - t_lo) / (t_hi - t_lo) * (lp_hi - lp_lo)
            };
            return 10f64.powf(lp);
        }
    }
    f64::NAN
}

/// Forecast max temperature (C) from a 100-hPa mixed layer
/// (port of `params.max_temp`).
pub fn max_temp(prof: &Profile) -> f64 {
    let mixlayer = prof.pres[prof.sfc] - 100.0;
    let temp = thermo::ctok(interp::temp(prof, mixlayer)) + 2.0;
    thermo::ktoc(temp * (prof.pres[prof.sfc] / mixlayer).powf(crate::constants::ROCP))
}

/// Mean mixing ratio (g/kg) in a layer, `exact = True` path
/// (port of `params.mean_mixratio`).
pub fn mean_mixratio_exact(prof: &Profile, pbot: f64, ptop: f64) -> f64 {
    let mut pbot = pbot;
    if !qc(interp::temp(prof, pbot)) {
        pbot = prof.pres[prof.sfc];
    }
    if !qc(interp::temp(prof, ptop)) {
        return f64::NAN;
    }
    let ind1 = match (0..prof.pres.len()).find(|&i| prof.pres[i].is_finite() && pbot > prof.pres[i])
    {
        Some(i) => i,
        None => return f64::NAN,
    };
    let ind2 = match (0..prof.pres.len())
        .rev()
        .find(|&i| prof.pres[i].is_finite() && ptop < prof.pres[i])
    {
        Some(i) => i,
        None => return f64::NAN,
    };
    let dwpt1 = interp::dwpt(prof, pbot);
    let dwpt2 = interp::dwpt(prof, ptop);
    let mut totd = dwpt1 + dwpt2;
    let mut totp = pbot + ptop;
    let mut count = 2.0;
    for i in ind1..=ind2 {
        if prof.dwpc[i].is_finite() && prof.pres[i].is_finite() {
            totd += 2.0 * prof.dwpc[i];
            totp += 2.0 * prof.pres[i];
            count += 2.0;
        }
    }
    thermo::mixratio(totp / count, totd / count)
}

/// Mean potential temperature (C) in a layer, `exact = True` path
/// (port of `params.mean_theta`).
pub fn mean_theta_exact(prof: &Profile, pbot: f64, ptop: f64) -> f64 {
    let mut pbot = pbot;
    if !qc(interp::temp(prof, pbot)) {
        pbot = prof.pres[prof.sfc];
    }
    if !qc(interp::temp(prof, ptop)) {
        return f64::NAN;
    }
    let ind1 = match (0..prof.pres.len()).find(|&i| prof.pres[i].is_finite() && pbot > prof.pres[i])
    {
        Some(i) => i,
        None => return f64::NAN,
    };
    let ind2 = match (0..prof.pres.len())
        .rev()
        .find(|&i| prof.pres[i].is_finite() && ptop < prof.pres[i])
    {
        Some(i) => i,
        None => return f64::NAN,
    };
    let th1 = thermo::theta(pbot, interp::temp(prof, pbot), 1000.0);
    let th2 = thermo::theta(ptop, interp::temp(prof, ptop), 1000.0);
    let mut tot = th1 + th2;
    let mut count = 2.0;
    for i in ind1..=ind2 {
        let th = thermo::theta(prof.pres[i], prof.tmpc[i], 1000.0);
        if th.is_finite() {
            tot += 2.0 * th;
            count += 2.0;
        }
    }
    tot / count
}

/// Mean theta-e (C) in a layer, non-exact path (port of `params.mean_thetae`).
pub fn mean_thetae(prof: &Profile, pbot: f64, ptop: f64) -> f64 {
    let mut pbot = pbot;
    if !qc(interp::temp(prof, pbot)) {
        pbot = prof.pres[prof.sfc];
    }
    if !qc(interp::temp(prof, ptop)) {
        return f64::NAN;
    }
    let ps = arange_desc(pbot, ptop - 1.0);
    let mut tot = 0.0;
    let mut wsum = 0.0;
    for &p in &ps {
        let te = interp::thetae(prof, p);
        if te.is_finite() {
            tot += te * p;
            wsum += p;
        }
    }
    if wsum == 0.0 {
        return f64::NAN;
    }
    tot / wsum
}

/// Pressure of the most unstable level between `pbot` and `ptop`
/// (port of `params.most_unstable_level`, non-exact path).
pub fn most_unstable_level(prof: &Profile, pbot: f64, ptop: f64) -> f64 {
    let mut pbot = pbot;
    if !qc(interp::temp(prof, pbot)) {
        pbot = prof.pres[prof.sfc];
    }
    if !qc(interp::temp(prof, ptop)) {
        return f64::NAN;
    }
    let ps = arange_desc(pbot, ptop - 1.0);
    let mut best = f64::NEG_INFINITY;
    let mut mts = Vec::with_capacity(ps.len());
    for &p in &ps {
        let t = interp::temp(prof, p);
        let d = interp::dwpt(prof, p);
        let (p2, t2) = thermo::drylift(p, t, d);
        // wetlift(p2, t2, 1000) == the saturated theta directly (satlift is a
        // no-op at 1000 hPa).
        let thta = thermo::theta(p2, t2, 1000.0);
        let mt = if thta.is_nan() {
            f64::NAN
        } else {
            thta - thermo::wobf(thta) + thermo::wobf(t2)
        };
        if mt.is_finite() && mt > best {
            best = mt;
        }
        mts.push(mt);
    }
    for (i, mt) in mts.iter().enumerate() {
        if (mt - best).abs() < TOL {
            return ps[i];
        }
    }
    f64::NAN
}

/// The lifting parcel level values for a parcel type (port of `DefineParcel`).
pub fn define_parcel(prof: &Profile, kind: ParcelType) -> (f64, f64, f64) {
    match kind {
        ParcelType::Surface => (
            prof.pres[prof.sfc],
            prof.tmpc[prof.sfc],
            prof.dwpc[prof.sfc],
        ),
        ParcelType::Forecast => {
            let tmpc = max_temp(prof);
            let pres = prof.pres[prof.sfc];
            let mmr = mean_mixratio_exact(prof, pres, pres - 100.0);
            let dwpc = thermo::temp_at_mixrat(mmr, pres);
            (pres, tmpc, dwpc)
        }
        ParcelType::MostUnstable => {
            let pbot = prof.pres[prof.sfc];
            let ptop = pbot - 300.0;
            let pres = most_unstable_level(prof, pbot, ptop);
            (pres, interp::temp(prof, pres), interp::dwpt(prof, pres))
        }
        ParcelType::MixedLayer => {
            let pbot = prof.pres[prof.sfc];
            let ptop = pbot - 100.0;
            let mtheta = mean_theta_exact(prof, pbot, ptop);
            let tmpc = thermo::theta(1000.0, mtheta, pbot);
            let mmr = mean_mixratio_exact(prof, pbot, ptop);
            let dwpc = thermo::temp_at_mixrat(mmr, pbot);
            (pbot, tmpc, dwpc)
        }
    }
}

/// Lightweight CAPE/CIN-only lift (port of `params.cape`, classic lifter).
/// Returns `(bplus, bminus)`; NaN when the parcel could not be lifted.
pub fn cape_lite(prof: &Profile, pres: f64, tmpc: f64, dwpc: f64) -> (f64, f64) {
    let mut pbot = prof.pres[prof.sfc];
    let ptop = *prof
        .pres
        .iter()
        .rev()
        .find(|p| p.is_finite())
        .unwrap_or(&f64::NAN);
    if pbot > pres {
        pbot = pres;
    }
    if !qc(interp::vtmp(prof, pbot)) || !qc(interp::vtmp(prof, ptop)) {
        return (f64::NAN, f64::NAN);
    }

    let mut totp = 0.0;
    let mut totn = 0.0;

    let (pe2, tp2_lcl) = thermo::drylift(pres, tmpc, dwpc);
    if !qc(pe2) {
        return (f64::NAN, f64::NAN);
    }
    let blupper = pe2;
    let theta_parcel = thermo::theta(pe2, tp2_lcl, 1000.0);
    let blmr = thermo::mixratio(pres, dwpc);

    // Accumulated CINH in the mixing layer below the LCL.
    let pp = arange_desc(pbot, blupper - 1.0);
    let mut prev: Option<(f64, f64)> = None; // (tdef, hh)
    for &p in &pp {
        let hh = interp::hght(prof, p);
        let env_theta = thermo::theta(p, interp::temp(prof, p), 1000.0);
        let env_dwpt = interp::dwpt(prof, p);
        let tv_env = thermo::virtemp(p, env_theta, env_dwpt);
        let tmp1 = thermo::virtemp(p, theta_parcel, thermo::temp_at_mixrat(blmr, p));
        let tdef = (tmp1 - tv_env) / thermo::ctok(tv_env);
        if let Some((tdef0, hh0)) = prev {
            let lyre = G * (tdef0 + tdef) / 2.0 * (hh - hh0);
            if lyre.is_finite() && lyre < 0.0 {
                totn += lyre;
            }
        }
        prev = Some((tdef, hh));
    }

    if pbot > pe2 {
        pbot = pe2;
    }
    let last_finite = prof.pres.iter().rev().find(|p| p.is_finite());
    match last_finite {
        Some(p) if pbot >= *p => {}
        _ => return (f64::NAN, f64::NAN),
    }

    // lptr: first index with pres < pbot (strict); uptr: last with pres > ptop.
    let lptr = match (0..prof.pres.len()).find(|&i| prof.pres[i].is_finite() && pbot > prof.pres[i])
    {
        Some(i) => i,
        None => return (f64::NAN, f64::NAN),
    };
    let uptr = match (0..prof.pres.len())
        .rev()
        .find(|&i| prof.pres[i].is_finite() && ptop < prof.pres[i])
    {
        Some(i) => i,
        None => prof.pres.len() - 1,
    };

    let mut pe1 = pbot;
    let mut h1 = interp::hght(prof, pe1);
    let mut te1 = interp::vtmp(prof, pe1);
    let mut tp1 = tp2_lcl;
    let mut bplus = f64::NAN;
    let mut bminus = f64::NAN;
    let bplus_set = false;

    for i in lptr..prof.pres.len() {
        if !qc(prof.tmpc[i]) {
            continue;
        }
        let pe2 = prof.pres[i];
        let h2 = prof.hght[i];
        let te2 = prof.vtmp[i];
        let tp2 = thermo::wetlift(pe1, tp1, pe2);
        let tdef1 = (thermo::virtemp(pe1, tp1, tp1) - te1) / thermo::ctok(te1);
        let tdef2 = (thermo::virtemp(pe2, tp2, tp2) - te2) / thermo::ctok(te2);
        let lyre = G * (tdef1 + tdef2) / 2.0 * (h2 - h1);

        if lyre > 0.0 {
            totp += lyre;
        } else if pe2 > 500.0 {
            totn += lyre;
        }

        pe1 = pe2;
        h1 = h2;
        te1 = te2;
        tp1 = tp2;

        if i >= uptr && !bplus_set {
            let pe3 = pe1;
            let h3 = h1;
            let te3 = te1;
            let tp3 = tp1;
            let lyrf = lyre;
            if lyrf > 0.0 {
                bplus = totp - lyrf;
                bminus = totn;
            } else {
                bplus = totp;
                if pe2 > 500.0 {
                    bminus = totn + lyrf;
                } else {
                    bminus = totn;
                }
            }
            let pe2b = ptop;
            let h2b = interp::hght(prof, pe2b);
            let te2b = interp::vtmp(prof, pe2b);
            let tp2b = thermo::wetlift(pe3, tp3, pe2b);
            let tdef3 = (thermo::virtemp(pe3, tp3, tp3) - te3) / thermo::ctok(te3);
            let tdef2b = (thermo::virtemp(pe2b, tp2b, tp2b) - te2b) / thermo::ctok(te2b);
            let lyrf = G * (tdef3 + tdef2b) / 2.0 * (h2b - h3);
            if lyrf > 0.0 {
                bplus += lyrf;
            } else if pe2b > 500.0 {
                bminus += lyrf;
            }
            if bplus == 0.0 {
                bminus = 0.0;
            }
            break;
        }
    }
    (bplus, bminus)
}

/// Full parcel lift (port of `params.parcelx` with default layer bounds).
pub fn parcelx(prof: &Profile, kind: ParcelType) -> Parcel {
    let (pres, tmpc, dwpc) = define_parcel(prof, kind);
    parcelx_at(prof, pres, tmpc, dwpc)
}

/// Full parcel lift of an explicit parcel (pressure hPa, temp/dewpoint C).
pub fn parcelx_at(prof: &Profile, pres: f64, tmpc: f64, dwpc: f64) -> Parcel {
    let mut pcl = Parcel {
        lpl_pres: pres,
        lpl_tmpc: tmpc,
        lpl_dwpc: dwpc,
        ..Parcel::default()
    };
    if !prof.pres.iter().any(|p| p.is_finite()) {
        return pcl;
    }

    let mut totp = 0.0;
    let mut totn = 0.0;
    let mut tote = 0.0;
    let mut li_max = -9999.0;

    let mut pbot = prof.pres[prof.sfc];
    let ptop = *prof
        .pres
        .iter()
        .rev()
        .find(|p| p.is_finite())
        .unwrap_or(&f64::NAN);
    if pbot > pres {
        pbot = pres;
    }

    // Mixing layer up to the LCL.
    let mut pe1 = pbot;
    let tp1_init = thermo::virtemp(pres, tmpc, dwpc);
    let mut ttrace = vec![tp1_init];
    let mut ptrace = vec![pe1];

    let (pe2, tp2) = thermo::drylift(pres, tmpc, dwpc);
    if !qc(pe2) {
        return pcl;
    }
    let blupper = pe2;
    pcl.lclpres = pe2.min(prof.pres[prof.sfc]);
    pcl.lclhght = interp::to_agl(prof, interp::hght(prof, pe2));
    ptrace.push(pe2);
    ttrace.push(thermo::virtemp(pe2, tp2, tp2));

    let theta_parcel = thermo::theta(pe2, tp2, 1000.0);
    let blmr = thermo::mixratio(pres, dwpc);

    // Accumulated CINH in the mixing layer below the LCL.
    let pp = arange_desc(pbot, blupper - 1.0);
    let mut prev: Option<(f64, f64)> = None;
    for &p in &pp {
        let hh = interp::hght(prof, p);
        let env_theta = thermo::theta(p, interp::temp(prof, p), 1000.0);
        let env_dwpt = interp::dwpt(prof, p);
        let tv_env = thermo::virtemp(p, env_theta, env_dwpt);
        let tmp1 = thermo::virtemp(p, theta_parcel, thermo::temp_at_mixrat(blmr, p));
        let tdef = (tmp1 - tv_env) / thermo::ctok(tv_env);
        if let Some((tdef0, hh0)) = prev {
            let lyre = G * (tdef0 + tdef) / 2.0 * (hh - hh0);
            if lyre.is_finite() && lyre < 0.0 {
                totn += lyre;
            }
        }
        prev = Some((tdef, hh));
    }

    if pbot > pe2 {
        pbot = pe2;
    }

    // Environment temperature levels.
    pcl.p0c = temp_lvl(prof, 0.0);
    pcl.pm10c = temp_lvl(prof, -10.0);
    pcl.pm20c = temp_lvl(prof, -20.0);
    pcl.pm30c = temp_lvl(prof, -30.0);
    pcl.hght0c = interp::hght(prof, pcl.p0c);
    pcl.hghtm20c = interp::hght(prof, pcl.pm20c);
    pcl.hghtm30c = interp::hght(prof, pcl.pm30c);

    let last_finite = prof.pres.iter().rev().find(|p| p.is_finite());
    match last_finite {
        Some(p) if pbot >= *p => {}
        _ => return pcl,
    }

    // lptr: first index with pbot >= pres; uptr: last index with ptop <= pres.
    let lptr = match (0..prof.pres.len())
        .find(|&i| prof.pres[i].is_finite() && pbot >= prof.pres[i])
    {
        Some(i) => i,
        None => return pcl,
    };
    let uptr = match (0..prof.pres.len())
        .rev()
        .find(|&i| prof.pres[i].is_finite() && ptop <= prof.pres[i])
    {
        Some(i) => i,
        None => prof.pres.len() - 1,
    };

    // Moist ascent from the LCL.
    pe1 = pbot;
    let mut h1 = interp::hght(prof, pe1);
    let mut te1 = interp::vtmp(prof, pe1);
    let mut tp1 = thermo::wetlift(pe2, tp2, pe1);
    let mut lyre = 0.0;
    let mut lyrlast;
    let mut pelast;
    let mut bplus_set = false;
    let mut lfc_set = false;
    let mut el_set = false;
    let mut mpl_set = false;

    let ntr = prof.pres.len() - lptr;
    let mut ttraces = vec![f64::NAN; ntr];
    let mut ptraces = vec![f64::NAN; ntr];

    for i in lptr..prof.pres.len() {
        if !qc(prof.tmpc[i]) {
            continue;
        }
        let mut pe2 = prof.pres[i];
        let mut h2 = prof.hght[i];
        let mut te2 = prof.vtmp[i];
        let mut tp2 = thermo::wetlift(pe1, tp1, pe2);
        let tdef1 = (thermo::virtemp(pe1, tp1, tp1) - te1) / thermo::ctok(te1);
        let tdef2 = (thermo::virtemp(pe2, tp2, tp2) - te2) / thermo::ctok(te2);

        ptraces[i - lptr] = pe2;
        ttraces[i - lptr] = thermo::virtemp(pe2, tp2, tp2);
        lyrlast = lyre;
        lyre = G * (tdef1 + tdef2) / 2.0 * (h2 - h1);

        if lyre > 0.0 {
            totp += lyre;
        } else if pe2 > 500.0 {
            totn += lyre;
        }

        // Max lifted index.
        let mli = thermo::virtemp(pe2, tp2, tp2) - te2;
        if mli > li_max {
            li_max = mli;
        }

        tote += lyre;
        pelast = pe1;
        pe1 = pe2;
        te1 = te2;
        tp1 = tp2;

        // Top of the specified layer.
        if i >= uptr && !bplus_set {
            let pe3 = pe1;
            let h3 = h2;
            let te3 = te1;
            let tp3 = tp1;
            let lyrf = lyre;
            if lyrf > 0.0 {
                pcl.bplus = totp - lyrf;
                pcl.bminus = totn;
            } else {
                pcl.bplus = totp;
                if pe2 > 500.0 {
                    pcl.bminus = totn + lyrf;
                } else {
                    pcl.bminus = totn;
                }
            }
            pe2 = ptop;
            h2 = interp::hght(prof, pe2);
            te2 = interp::vtmp(prof, pe2);
            tp2 = thermo::wetlift(pe3, tp3, pe2);
            let tdef3 = (thermo::virtemp(pe3, tp3, tp3) - te3) / thermo::ctok(te3);
            let tdef2b = (thermo::virtemp(pe2, tp2, tp2) - te2) / thermo::ctok(te2);
            let lyrf = G * (tdef3 + tdef2b) / 2.0 * (h2 - h3);
            if lyrf > 0.0 {
                pcl.bplus += lyrf;
            } else if pe2 > 500.0 {
                pcl.bminus += lyrf;
            }
            if pcl.bplus == 0.0 {
                pcl.bminus = 0.0;
            }
            bplus_set = true;
        }

        h1 = h2;

        // LFC possibility.
        if lyre >= 0.0 && lyrlast <= 0.0 {
            let tp3 = tp1;
            let pe2c = pe1;
            let mut pe3 = pelast;
            let lifted_vt = |pe: f64| {
                let w = thermo::wetlift(pe2c, tp3, pe);
                thermo::virtemp(pe, w, w)
            };
            if interp::vtmp(prof, pe3) < lifted_vt(pe3) {
                pcl.lfcpres = pe3;
                pcl.lfchght = interp::to_agl(prof, interp::hght(prof, pe3));
                pcl.elpres = f64::NAN;
                pcl.elhght = f64::NAN;
                pcl.mplpres = f64::NAN;
                el_set = false;
                mpl_set = false;
                lfc_set = true;
            } else {
                while interp::vtmp(prof, pe3) > lifted_vt(pe3) && pe3 > 0.0 {
                    pe3 -= 5.0;
                }
                if pe3 > 0.0 {
                    pcl.lfcpres = pe3;
                    pcl.lfchght = interp::to_agl(prof, interp::hght(prof, pe3));
                    tote = 0.0;
                    li_max = -9999.0;
                    pcl.elpres = f64::NAN;
                    pcl.elhght = f64::NAN;
                    pcl.mplpres = f64::NAN;
                    el_set = false;
                    mpl_set = false;
                    lfc_set = true;
                }
            }
            // Force the LFC to be at least at the LCL.
            if lfc_set && qc(pcl.lfcpres) && qc(pcl.lclpres) && pcl.lfcpres >= pcl.lclpres {
                pcl.lfcpres = pcl.lclpres;
                pcl.lfchght = pcl.lclhght;
            }
        }

        // EL possibility.
        if lyre <= 0.0 && lyrlast >= 0.0 {
            let tp3 = tp1;
            let pe2c = pe1;
            let mut pe3 = pelast;
            let lifted_vt = |pe: f64| {
                let w = thermo::wetlift(pe2c, tp3, pe);
                thermo::virtemp(pe, w, w)
            };
            while interp::vtmp(prof, pe3) < lifted_vt(pe3) && pe3 > 0.0 {
                pe3 -= 5.0;
            }
            pcl.elpres = pe3;
            pcl.elhght = interp::to_agl(prof, interp::hght(prof, pcl.elpres));
            pcl.mplpres = f64::NAN;
            mpl_set = false;
            el_set = true;
        }

        // MPL possibility.
        if tote < 0.0 && !mpl_set && el_set && qc(pcl.elpres) {
            let mut pe3 = pelast;
            let mut h3 = interp::hght(prof, pe3);
            let mut te3 = interp::vtmp(prof, pe3);
            let mut tp3 = thermo::wetlift(pe1, tp1, pe3);
            let mut totx = tote - lyre;
            let mut pe2m = pelast;
            while totx > 0.0 && pe2m > 2.0 {
                pe2m -= 1.0;
                let te2m = interp::vtmp(prof, pe2m);
                let tp2m = thermo::wetlift(pe3, tp3, pe2m);
                let h2m = interp::hght(prof, pe2m);
                let tdef3 = (thermo::virtemp(pe3, tp3, tp3) - te3) / thermo::ctok(te3);
                let tdef2m = (thermo::virtemp(pe2m, tp2m, tp2m) - te2m) / thermo::ctok(te2m);
                let lyrf = G * (tdef3 + tdef2m) / 2.0 * (h2m - h3);
                if lyrf.is_nan() {
                    break;
                }
                totx += lyrf;
                tp3 = tp2m;
                te3 = te2m;
                pe3 = pe2m;
                h3 = h2m;
            }
            pcl.mplpres = pe2m;
            mpl_set = true;
        }
    }

    if !bplus_set && !qc(pcl.bplus) {
        pcl.bplus = totp;
    }
    if pcl.bplus.is_finite() && pcl.bplus.floor() == 0.0 {
        pcl.bminus = 0.0;
    }

    let mut full_p = ptrace;
    full_p.extend_from_slice(&ptraces);
    let mut full_t = ttrace;
    full_t.extend_from_slice(&ttraces);
    pcl.ptrace = full_p;
    pcl.ttrace = full_t;
    pcl
}

/// Effective inflow layer `(pbot, ptop)` in hPa (port of
/// `params.effective_inflow_layer`, linear search).
pub fn effective_inflow_layer(prof: &Profile, ecape: f64, ecinh: f64, mupcl: &Parcel) -> (f64, f64) {
    let mucape = mupcl.bplus;
    let mucinh = mupcl.bminus;
    let mut pbot = f64::NAN;
    let mut ptop = f64::NAN;
    if !(mucape != 0.0) {
        return (pbot, ptop);
    }
    if mucape >= ecape && mucinh > ecinh {
        let mut bptr = None;
        for i in prof.sfc..prof.top {
            let (bplus, bminus) = cape_lite(prof, prof.pres[i], prof.tmpc[i], prof.dwpc[i]);
            if bplus >= ecape && bminus > ecinh {
                pbot = prof.pres[i];
                bptr = Some(i);
                break;
            }
        }
        let bptr = match bptr {
            Some(i) => i,
            None => return (f64::NAN, f64::NAN),
        };
        if !qc(pbot) {
            return (f64::NAN, f64::NAN);
        }
        for i in bptr + 1..prof.top {
            // Python truthiness: masked or exactly-zero values are skipped.
            if !qc(prof.dwpc[i]) || prof.dwpc[i] == 0.0 || !qc(prof.tmpc[i]) || prof.tmpc[i] == 0.0
            {
                continue;
            }
            let (bplus, bminus) = cape_lite(prof, prof.pres[i], prof.tmpc[i], prof.dwpc[i]);
            // NaN comparisons are false, matching numpy masked semantics.
            if bplus < ecape || bminus <= ecinh {
                let mut j = 1;
                while i >= j && !qc(prof.dwpc[i - j]) && !qc(prof.tmpc[i - j]) {
                    j += 1;
                }
                ptop = prof.pres[i - j];
                if ptop > pbot {
                    ptop = pbot;
                }
                break;
            }
        }
    }
    (pbot, ptop)
}

/// Bunkers storm motion `(rstu, rstv, lstu, lstv)` in kts (port of
/// `params.bunkers_storm_motion` given an MU parcel and effective-layer base).
pub fn bunkers_storm_motion(prof: &Profile, mupcl: &Parcel, pbot: f64) -> (f64, f64, f64, f64) {
    let d = ms2kts(7.5);
    let mucape = mupcl.bplus;
    let muel = mupcl.elhght;
    let base = interp::to_agl(prof, interp::hght(prof, pbot));
    if mucape > 100.0 && qc(muel) && qc(base) {
        let depth = muel - base;
        let htop = base + depth * (65.0 / 100.0);
        let ptop = interp::pres(prof, interp::to_msl(prof, htop));
        let (mnu, mnv) = winds::mean_wind(prof, pbot, ptop, 0.0, 0.0);
        let (sru, srv) = winds::wind_shear(prof, pbot, ptop);
        let srmag = mag(sru, srv);
        let uchg = d / srmag * srv;
        let vchg = d / srmag * sru;
        (mnu + uchg, mnv - vchg, mnu - uchg, mnv + vchg)
    } else {
        winds::non_parcel_bunkers_motion(prof)
    }
}

/// Maximum lapse rate over `depth`-deep layers between `lower` and `upper`
/// (m AGL). Returns `(lapse_rate C/km, pbot hPa, ptop hPa)`
/// (port of `params.max_lapse_rate`).
pub fn max_lapse_rate(
    prof: &Profile,
    lower: f64,
    upper: f64,
    interval: f64,
    depth: f64,
) -> (f64, f64, f64) {
    let mut best = f64::NEG_INFINITY;
    let mut best_pb = f64::NAN;
    let mut best_pt = f64::NAN;
    let mut z = lower;
    while z <= upper - depth + 1e-9 {
        let zbot = interp::to_msl(prof, z);
        let ztop = interp::to_msl(prof, z + depth);
        let pb = interp::pres(prof, zbot);
        let pt = interp::pres(prof, ztop);
        let lr = (interp::vtmp(prof, pt) - interp::vtmp(prof, pb)) * -1000.0;
        if lr.is_finite() && lr > best {
            best = lr;
            best_pb = pb;
            best_pt = pt;
        }
        z += interval;
    }
    if !best.is_finite() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    (best / depth, best_pb, best_pt)
}

/// DCAPE (J/kg) plus the downdraft parcel trace (port of `params.dcape`).
/// Returns `(dcape, ttrace, ptrace)`.
pub fn dcape(prof: &Profile) -> (f64, Vec<f64>, Vec<f64>) {
    let sfc_pres = prof.pres[prof.sfc];
    // Filter levels where theta-e or pressure is masked.
    let mut pres = Vec::new();
    let mut hght = Vec::new();
    let mut tmpc = Vec::new();
    for i in 0..prof.pres.len() {
        if prof.thetae[i].is_finite() && prof.pres[i].is_finite() {
            pres.push(prof.pres[i]);
            hght.push(prof.hght[i]);
            tmpc.push(prof.tmpc[i]);
        }
    }
    if pres.is_empty() {
        return (f64::NAN, Vec::new(), Vec::new());
    }

    // Minimum 100-hPa layer averaged theta-e in the lowest 400 hPa.
    let mut mine = 1000.0;
    let mut minp = -999.0;
    for i in 0..pres.len() {
        if pres[i] >= sfc_pres - 400.0 {
            let te = mean_thetae(prof, pres[i], pres[i] - 100.0);
            if qc(te) && te < mine {
                minp = pres[i] - 50.0;
                mine = te;
            }
        }
    }
    if minp < 0.0 {
        return (f64::NAN, Vec::new(), Vec::new());
    }
    let upper = minp;

    // Last index with pres >= upper.
    let uptr = match (0..pres.len()).rev().find(|&i| pres[i] >= upper) {
        Some(i) => i,
        None => return (f64::NAN, Vec::new(), Vec::new()),
    };

    let mut tp1 = thermo::wetbulb(upper, interp::temp(prof, upper), interp::dwpt(prof, upper));
    let mut pe1 = upper;
    let mut te1 = interp::temp(prof, pe1);
    let mut h1 = interp::hght(prof, pe1);
    let mut tote = 0.0;

    let mut ttrace = vec![tp1];
    let mut ptrace = vec![upper];
    let mut ttraces = vec![f64::NAN; uptr + 1];
    let mut ptraces = vec![f64::NAN; uptr + 1];

    for i in (0..=uptr).rev() {
        let pe2 = pres[i];
        let te2 = tmpc[i];
        let h2 = hght[i];
        let tp2 = thermo::wetlift(pe1, tp1, pe2);
        if qc(te1) && qc(te2) {
            let tdef1 = (tp1 - te1) / thermo::ctok(te1);
            let tdef2 = (tp2 - te2) / thermo::ctok(te2);
            let lyre = 9.8 * (tdef1 + tdef2) / 2.0 * (h2 - h1);
            if lyre.is_finite() {
                tote += lyre;
            }
        }
        ttraces[i] = tp2;
        ptraces[i] = pe2;
        pe1 = pe2;
        te1 = te2;
        h1 = h2;
        tp1 = tp2;
    }
    ttraces.reverse();
    ptraces.reverse();
    ttrace.extend_from_slice(&ttraces);
    ptrace.extend_from_slice(&ptraces);
    (tote, ttrace, ptrace)
}
