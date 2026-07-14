//! Sounding profile container (port of `sharppy.sharptab.profile`).
//!
//! [`SoundingData`] holds the raw sounding arrays; [`Profile::new`] derives
//! everything the skew-T needs (virtual temperature, wetbulb, theta-e,
//! parcels, effective inflow layer, storm motion, ESRH, DCAPE trace, max
//! lapse rate) exactly like the SHARPpy `ConvectiveProfile`.

use crate::constants::MISSING;
use crate::params::{self, Parcel, ParcelType};
use crate::utils::{qc, vec2comp};
use crate::{interp, thermo, winds};

/// Raw sounding input. All slices must be the same length, ordered from the
/// surface upward (decreasing pressure). Missing values may be encoded as
/// `missing` (default -9999), NaN, or anything non-finite.
#[derive(Clone, Debug, Default)]
pub struct SoundingData {
    /// Pressure (hPa).
    pub pres: Vec<f64>,
    /// Height (m MSL).
    pub hght: Vec<f64>,
    /// Temperature (C).
    pub tmpc: Vec<f64>,
    /// Dewpoint (C).
    pub dwpc: Vec<f64>,
    /// Wind direction (degrees from north).
    pub wdir: Vec<f64>,
    /// Wind speed (kts).
    pub wspd: Vec<f64>,
    /// Pressure vertical velocity omega (Pa/s), optional.
    pub omeg: Option<Vec<f64>>,
    /// Latitude (degrees); used for the hemisphere of wind barbs.
    pub latitude: Option<f64>,
    /// Missing-data sentinel (default -9999.0).
    pub missing: Option<f64>,
}

/// A fully analyzed profile ready to hand to the [`crate::SkewT`] widget.
#[derive(Clone, Debug)]
pub struct Profile {
    pub pres: Vec<f64>,
    pub hght: Vec<f64>,
    pub tmpc: Vec<f64>,
    pub dwpc: Vec<f64>,
    pub wdir: Vec<f64>,
    pub wspd: Vec<f64>,
    pub u: Vec<f64>,
    pub v: Vec<f64>,
    /// Omega (Pa/s); empty when not supplied.
    pub omeg: Vec<f64>,
    /// log10(pres) per level.
    pub logp: Vec<f64>,
    /// Virtual temperature (C) per level.
    pub vtmp: Vec<f64>,
    /// Wetbulb temperature (C) per level.
    pub wetbulb: Vec<f64>,
    /// Potential temperature (C) per level.
    pub theta: Vec<f64>,
    /// Equivalent potential temperature (C) per level.
    pub thetae: Vec<f64>,
    /// Water vapor mixing ratio (g/kg) per level.
    pub wvmr: Vec<f64>,
    /// Index of the surface (lowest level with a valid temperature).
    pub sfc: usize,
    /// Index of the profile top (highest level with a valid temperature).
    pub top: usize,
    pub latitude: f64,

    /// Surface-based parcel.
    pub sfcpcl: Parcel,
    /// Forecast surface parcel.
    pub fcstpcl: Parcel,
    /// Most unstable parcel (lowest 400 hPa).
    pub mupcl: Parcel,
    /// 100-hPa mixed layer parcel.
    pub mlpcl: Parcel,

    /// Effective inflow layer bottom pressure (hPa; NaN when absent).
    pub ebottom: f64,
    /// Effective inflow layer top pressure (hPa; NaN when absent).
    pub etop: f64,
    /// Effective inflow layer bottom height (m AGL; NaN when absent).
    pub ebotm: f64,
    /// Effective inflow layer top height (m AGL; NaN when absent).
    pub etopm: f64,
    /// Bunkers storm motion `(right_u, right_v, left_u, left_v)` (kts).
    pub srwind: (f64, f64, f64, f64),
    /// Effective SRH (right mover) (m2/s2).
    pub right_esrh: f64,
    /// Max 2-6 km AGL lapse rate: `(value C/km, pbot hPa, ptop hPa)`.
    pub max_lapse_rate_2_6: (f64, f64, f64),
    /// DCAPE (J/kg).
    pub dcape: f64,
    /// Downdraft parcel trace temperatures (C).
    pub dpcl_ttrace: Vec<f64>,
    /// Downdraft parcel trace pressures (hPa).
    pub dpcl_ptrace: Vec<f64>,
}

fn clean(mut v: Vec<f64>, missing: f64) -> Vec<f64> {
    for x in v.iter_mut() {
        if !x.is_finite() || (*x - missing).abs() < 1e-6 || *x <= -9990.0 {
            *x = f64::NAN;
        }
    }
    v
}

impl Profile {
    /// Analyze a raw sounding. Returns `None` when the input arrays are
    /// mismatched in length or no valid temperature level exists.
    pub fn new(data: SoundingData) -> Option<Profile> {
        let n = data.pres.len();
        if n == 0
            || data.hght.len() != n
            || data.tmpc.len() != n
            || data.dwpc.len() != n
            || data.wdir.len() != n
            || data.wspd.len() != n
        {
            return None;
        }
        let missing = data.missing.unwrap_or(MISSING);
        let pres = clean(data.pres, missing);
        let hght = clean(data.hght, missing);
        let tmpc = clean(data.tmpc, missing);
        let dwpc = clean(data.dwpc, missing);
        let wdir = clean(data.wdir, missing);
        let wspd = clean(data.wspd, missing);
        let omeg = match data.omeg {
            Some(o) if o.len() == n => clean(o, missing),
            _ => Vec::new(),
        };

        let sfc = tmpc.iter().position(|t| qc(*t))?;
        let top = n - 1 - tmpc.iter().rev().position(|t| qc(*t))?;

        let mut u = vec![f64::NAN; n];
        let mut v = vec![f64::NAN; n];
        for i in 0..n {
            if qc(wdir[i]) && qc(wspd[i]) {
                let (uu, vv) = vec2comp(wdir[i], wspd[i]);
                u[i] = uu;
                v[i] = vv;
            }
        }
        let logp: Vec<f64> = pres.iter().map(|p| p.log10()).collect();
        let mut vtmp = vec![f64::NAN; n];
        let mut wetbulb = vec![f64::NAN; n];
        let mut theta_arr = vec![f64::NAN; n];
        let mut thetae_arr = vec![f64::NAN; n];
        let mut wvmr = vec![f64::NAN; n];
        for i in 0..n {
            if qc(pres[i]) && qc(tmpc[i]) {
                vtmp[i] = thermo::virtemp(pres[i], tmpc[i], dwpc[i]);
                // SHARPpy stores the theta / theta-e profiles in Kelvin.
                theta_arr[i] = thermo::ctok(thermo::theta(pres[i], tmpc[i], 1000.0));
                if qc(dwpc[i]) {
                    wetbulb[i] = thermo::wetbulb(pres[i], tmpc[i], dwpc[i]);
                    thetae_arr[i] = thermo::ctok(thermo::thetae(pres[i], tmpc[i], dwpc[i]));
                    wvmr[i] = thermo::mixratio(pres[i], dwpc[i]);
                }
            }
        }

        let mut prof = Profile {
            pres,
            hght,
            tmpc,
            dwpc,
            wdir,
            wspd,
            u,
            v,
            omeg,
            logp,
            vtmp,
            wetbulb,
            theta: theta_arr,
            thetae: thetae_arr,
            wvmr,
            sfc,
            top,
            latitude: data.latitude.unwrap_or(35.0),
            sfcpcl: Parcel::default(),
            fcstpcl: Parcel::default(),
            mupcl: Parcel::default(),
            mlpcl: Parcel::default(),
            ebottom: f64::NAN,
            etop: f64::NAN,
            ebotm: f64::NAN,
            etopm: f64::NAN,
            srwind: (f64::NAN, f64::NAN, f64::NAN, f64::NAN),
            right_esrh: f64::NAN,
            max_lapse_rate_2_6: (f64::NAN, f64::NAN, f64::NAN),
            dcape: f64::NAN,
            dpcl_ttrace: Vec::new(),
            dpcl_ptrace: Vec::new(),
        };

        // Parcels (same set the SPC window lifts).
        prof.mupcl = params::parcelx(&prof, ParcelType::MostUnstable);
        prof.sfcpcl = if prof.mupcl.lpl_pres == prof.pres[prof.sfc] {
            prof.mupcl.clone()
        } else {
            params::parcelx(&prof, ParcelType::Surface)
        };
        prof.fcstpcl = params::parcelx(&prof, ParcelType::Forecast);
        prof.mlpcl = params::parcelx(&prof, ParcelType::MixedLayer);

        // Effective inflow layer + kinematics.
        let (ebot, etop) = params::effective_inflow_layer(&prof, 100.0, -250.0, &prof.mupcl);
        prof.ebottom = ebot;
        prof.etop = etop;
        if qc(ebot) && qc(etop) {
            prof.ebotm = interp::to_agl(&prof, interp::hght(&prof, ebot));
            prof.etopm = interp::to_agl(&prof, interp::hght(&prof, etop));
            prof.srwind = params::bunkers_storm_motion(&prof, &prof.mupcl, prof.ebottom);
            let (esrh, _, _) = winds::helicity(
                &prof,
                prof.ebotm,
                prof.etopm,
                prof.srwind.0,
                prof.srwind.1,
            );
            prof.right_esrh = esrh;
        } else {
            prof.srwind = winds::non_parcel_bunkers_motion(&prof);
        }

        prof.max_lapse_rate_2_6 = params::max_lapse_rate(&prof, 2000.0, 6000.0, 250.0, 2000.0);

        let (dcape_val, dttrace, dptrace) = params::dcape(&prof);
        prof.dcape = dcape_val;
        prof.dpcl_ttrace = dttrace;
        prof.dpcl_ptrace = dptrace;

        Some(prof)
    }

    /// The parcel of the given type (already computed).
    pub fn parcel(&self, kind: ParcelType) -> &Parcel {
        match kind {
            ParcelType::Surface => &self.sfcpcl,
            ParcelType::Forecast => &self.fcstpcl,
            ParcelType::MostUnstable => &self.mupcl,
            ParcelType::MixedLayer => &self.mlpcl,
        }
    }
}
