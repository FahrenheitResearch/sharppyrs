//! All scalar/vector parameters the full SPC window displays, computed once
//! from a [`crate::Profile`]. Values that `sharprs` provides directly are
//! wired here; the rest are ported from `sharpmod.sharptab.derived` /
//! vendored `sharppy.sharptab.params`. NaN = unavailable (renders as `--`).
//!
//! Golden reference for every field: `testdata/golden_full.json` (generated
//! by running the actual Python stack on the bundled example sounding).

use crate::Profile;

/// `(u, v)` kts.
pub type Comp = (f64, f64);
/// `(wdir deg, wspd kt)`.
pub type Vect = (f64, f64);

/// Every value the bottom index board / insets / hodograph annotations need.
#[derive(Clone, Debug)]
pub struct DerivedParams {
    // --- thermo block ---
    pub pwat: f64,
    pub k_idx: f64,
    pub tei: f64,
    pub esp: f64,
    pub mmp: f64,
    pub wndg: f64,
    pub dcp: f64,
    pub mburst: f64,
    pub ship: f64,
    pub right_scp: f64,
    pub left_scp: f64,
    pub stp_cin: f64,
    pub stp_fixed: f64,
    pub sweat: f64,
    pub sig_severe: f64,
    pub dcape: f64,
    pub drush_f: f64,
    pub mean_mixr: f64,
    pub low_rh: f64,
    pub mid_rh: f64,
    pub totals_totals: f64,
    pub conv_t_f: f64,
    pub max_t_f: f64,
    pub thetae_diff: f64,
    // --- lapse rates (C/km) ---
    pub lapserate_3km: f64,
    pub lapserate_3_6km: f64,
    pub lapserate_850_500: f64,
    pub lapserate_700_500: f64,
    pub lapserate_sfc_500m: f64,
    pub lapserate_sfc_1km: f64,
    // --- kinematics ---
    pub srh500: f64,
    pub srh1km: f64,
    pub srh3km: f64,
    pub right_esrh: f64,
    pub sfc_500m_shear: Comp,
    pub sfc_1km_shear: Comp,
    pub sfc_3km_shear: Comp,
    pub sfc_6km_shear: Comp,
    pub sfc_8km_shear: Comp,
    pub eff_shear: Comp,
    pub ebwd: Comp,
    pub lcl_el_shear: Comp,
    pub mean_wind_sfc_500m: Comp,
    pub mean_1km: Vect,
    pub mean_3km: Vect,
    pub mean_6km: Vect,
    pub mean_8km: Vect,
    pub mean_eff: Comp,
    pub mean_ebw: Comp,
    pub mean_lcl_el: Vect,
    pub srw_sfc_500m: Comp,
    pub srw_1km: Vect,
    pub srw_3km: Vect,
    pub srw_6km: Vect,
    pub srw_8km: Vect,
    pub srw_4_5km: Vect,
    pub srw_eff: Comp,
    pub srw_ebw: Comp,
    pub srw_lcl_el: Vect,
    pub wind1km: Vect,
    pub wind6km: Vect,
    /// Corfidi vectors: (upshear (u,v), downshear (u,v)).
    pub corfidi_up: Comp,
    pub corfidi_dn: Comp,
    pub right_critical_angle: f64,
    pub brnshear: f64,
    // --- SHARPpy-Reimagined derived composites ---
    pub ehi_0_1km: f64,
    pub ehi_0_3km: f64,
    pub vgp: f64,
    pub peskov: f64,
    pub mcs_index: f64,
    pub ncape: f64,
    pub lrghail: f64,
    pub lscp: f64,
    pub nstp: f64,
    pub hgz_cape: f64,
    pub wbz_height: f64,
    pub ecape: f64,
    pub modified_sherbe: f64,
    pub cape_0_3km: f64,
    pub cape_0_6km: f64,
    // --- strips / insets data ---
    /// Inferred temperature advection: (C/hr per layer, (pbot, ptop) bounds).
    pub temp_adv: Vec<f64>,
    pub temp_adv_bounds: Vec<(f64, f64)>,
    /// Storm slinky trajectory ((x, y) meters, updraft tilt deg) for the
    /// displayed parcel, right-mover storm motion.
    pub slinky_traj: Vec<(f64, f64)>,
    pub slinky_tilt: f64,
}

impl DerivedParams {
    pub fn nan() -> DerivedParams {
        DerivedParams {
            pwat: f64::NAN,
            k_idx: f64::NAN,
            tei: f64::NAN,
            esp: f64::NAN,
            mmp: f64::NAN,
            wndg: f64::NAN,
            dcp: f64::NAN,
            mburst: f64::NAN,
            ship: f64::NAN,
            right_scp: f64::NAN,
            left_scp: f64::NAN,
            stp_cin: f64::NAN,
            stp_fixed: f64::NAN,
            sweat: f64::NAN,
            sig_severe: f64::NAN,
            dcape: f64::NAN,
            drush_f: f64::NAN,
            mean_mixr: f64::NAN,
            low_rh: f64::NAN,
            mid_rh: f64::NAN,
            totals_totals: f64::NAN,
            conv_t_f: f64::NAN,
            max_t_f: f64::NAN,
            thetae_diff: f64::NAN,
            lapserate_3km: f64::NAN,
            lapserate_3_6km: f64::NAN,
            lapserate_850_500: f64::NAN,
            lapserate_700_500: f64::NAN,
            lapserate_sfc_500m: f64::NAN,
            lapserate_sfc_1km: f64::NAN,
            srh500: f64::NAN,
            srh1km: f64::NAN,
            srh3km: f64::NAN,
            right_esrh: f64::NAN,
            sfc_500m_shear: (f64::NAN, f64::NAN),
            sfc_1km_shear: (f64::NAN, f64::NAN),
            sfc_3km_shear: (f64::NAN, f64::NAN),
            sfc_6km_shear: (f64::NAN, f64::NAN),
            sfc_8km_shear: (f64::NAN, f64::NAN),
            eff_shear: (f64::NAN, f64::NAN),
            ebwd: (f64::NAN, f64::NAN),
            lcl_el_shear: (f64::NAN, f64::NAN),
            mean_wind_sfc_500m: (f64::NAN, f64::NAN),
            mean_1km: (f64::NAN, f64::NAN),
            mean_3km: (f64::NAN, f64::NAN),
            mean_6km: (f64::NAN, f64::NAN),
            mean_8km: (f64::NAN, f64::NAN),
            mean_eff: (f64::NAN, f64::NAN),
            mean_ebw: (f64::NAN, f64::NAN),
            mean_lcl_el: (f64::NAN, f64::NAN),
            srw_sfc_500m: (f64::NAN, f64::NAN),
            srw_1km: (f64::NAN, f64::NAN),
            srw_3km: (f64::NAN, f64::NAN),
            srw_6km: (f64::NAN, f64::NAN),
            srw_8km: (f64::NAN, f64::NAN),
            srw_4_5km: (f64::NAN, f64::NAN),
            srw_eff: (f64::NAN, f64::NAN),
            srw_ebw: (f64::NAN, f64::NAN),
            srw_lcl_el: (f64::NAN, f64::NAN),
            wind1km: (f64::NAN, f64::NAN),
            wind6km: (f64::NAN, f64::NAN),
            corfidi_up: (f64::NAN, f64::NAN),
            corfidi_dn: (f64::NAN, f64::NAN),
            right_critical_angle: f64::NAN,
            brnshear: f64::NAN,
            ehi_0_1km: f64::NAN,
            ehi_0_3km: f64::NAN,
            vgp: f64::NAN,
            peskov: f64::NAN,
            mcs_index: f64::NAN,
            ncape: f64::NAN,
            lrghail: f64::NAN,
            lscp: f64::NAN,
            nstp: f64::NAN,
            hgz_cape: f64::NAN,
            wbz_height: f64::NAN,
            ecape: f64::NAN,
            modified_sherbe: f64::NAN,
            cape_0_3km: f64::NAN,
            cape_0_6km: f64::NAN,
            temp_adv: Vec::new(),
            temp_adv_bounds: Vec::new(),
            slinky_traj: Vec::new(),
            slinky_tilt: f64::NAN,
        }
    }

    /// Compute everything from an analyzed profile.
    /// IMPLEMENTATION IN PROGRESS — see PORTING.md. Fields not yet wired
    /// return NaN and render as `--`.
    pub fn compute(prof: &Profile) -> DerivedParams {
        let _ = prof;
        DerivedParams::nan()
    }
}
