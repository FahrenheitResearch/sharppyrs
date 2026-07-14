//! Small numerics the display needs that `sharprs` does not (yet) provide.

use sharprs::params::cape::{self, LiftedParcelLevel, ParcelResult, ParcelType};
use sharprs::params::indices;
use sharprs::thermo;
use sharprs::winds;
use sharprs::Profile;

use crate::utils::qc;

/// SHARPpy-style forecast surface parcel: forecast max temperature (100-hPa
/// mixed layer warmed 2 K) with the mean boundary-layer mixing ratio.
/// sharprs's own `ParcelType::Forecast` uses the current surface temperature
/// instead; the original renderer's FCST row uses this definition.
pub fn forecast_parcel(inner: &Profile, cape_prof: &cape::Profile) -> ParcelResult {
    let pres = inner.pres[inner.sfc];
    let tmpc = indices::max_temp(inner, Some(100.0)).unwrap_or(f64::NAN);
    let mmr = indices::mean_mixratio(inner, Some(pres), Some(pres - 100.0)).unwrap_or(f64::NAN);
    let dwpc = thermo::temp_at_mixrat(mmr, pres);
    let lpl = LiftedParcelLevel {
        pres,
        tmpc,
        dwpc,
        parcel_type: ParcelType::UserDefined { pres, tmpc, dwpc },
    };
    cape::parcelx(cape_prof, &lpl, None, None)
}

/// Parcel-based Bunkers storm motion `(rstu, rstv, lstu, lstv)` in kts —
/// the Bunkers et al. 2014 method the original renderer uses (effective
/// inflow base to 65% of the MU parcel EL, pressure-weighted mean wind),
/// falling back to the non-parcel method (port of SHARPpy
/// `params.bunkers_storm_motion`).
pub fn bunkers_storm_motion(
    prof: &Profile,
    mupcl: &ParcelResult,
    pbot: f64,
) -> (f64, f64, f64, f64) {
    let d = 7.5 * 1.94384449; // 7.5 m/s deviation, in kts
    let mucape = mupcl.bplus;
    let muel = mupcl.elhght;
    let base = prof.to_agl(prof.interp_hght(pbot));
    if mucape > 100.0 && qc(muel) && qc(base) {
        let depth = muel - base;
        let htop = base + depth * (65.0 / 100.0);
        let ptop = prof.pres_at_height(prof.to_msl(htop));
        let (mnu, mnv) = winds::mean_wind(prof, pbot, ptop, -1.0, 0.0, 0.0)
            .unwrap_or((f64::NAN, f64::NAN));
        let (sru, srv) = winds::wind_shear(prof, pbot, ptop).unwrap_or((f64::NAN, f64::NAN));
        let srmag = (sru * sru + srv * srv).sqrt();
        let uchg = d / srmag * srv;
        let vchg = d / srmag * sru;
        (mnu + uchg, mnv - vchg, mnu - uchg, mnv + vchg)
    } else {
        winds::non_parcel_bunkers_motion(prof)
            .unwrap_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN))
    }
}
