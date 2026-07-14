//! Golden tests: the Rust port must reproduce the numbers computed by the
//! vendored SHARPpy 1.4.0a5 Python code (`testdata/golden.json`, generated
//! from the SHARPpy-Reimagined example HRRR sounding).

use serde_json::Value;
use sharppyrs::{Profile, SoundingData};

fn load() -> (Value, Profile) {
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/testdata/golden.json"
    ))
    .unwrap();
    let g: Value = serde_json::from_str(&raw).unwrap();
    let arr = |k: &str| -> Vec<f64> {
        g["input"][k]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap_or(f64::NAN))
            .collect()
    };
    let data = SoundingData {
        pres: arr("pres"),
        hght: arr("hght"),
        tmpc: arr("tmpc"),
        dwpc: arr("dwpc"),
        wdir: arr("wdir"),
        wspd: arr("wspd"),
        omeg: Some(arr("omeg")),
        latitude: g["latitude"].as_f64(),
        missing: None,
    };
    let prof = Profile::new(data).unwrap();
    (g, prof)
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

fn assert_close(actual: f64, expected: f64, tol: f64, what: &str) {
    if expected.is_nan() {
        assert!(
            actual.is_nan(),
            "{what}: expected missing, got {actual}"
        );
        return;
    }
    let denom = expected.abs().max(1.0);
    assert!(
        (actual - expected).abs() / denom <= tol,
        "{what}: got {actual}, expected {expected}"
    );
}

fn assert_arr_close(actual: &[f64], expected: &Value, tol: f64, what: &str) {
    let exp: Vec<f64> = expected
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap_or(f64::NAN))
        .collect();
    assert_eq!(actual.len(), exp.len(), "{what}: length mismatch");
    for (i, (a, e)) in actual.iter().zip(exp.iter()).enumerate() {
        assert_close(*a, *e, tol, &format!("{what}[{i}]"));
    }
}

#[test]
fn thermo_point_checks() {
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/testdata/golden.json"
    ))
    .unwrap();
    let g: Value = serde_json::from_str(&raw).unwrap();
    let t = &g["thermo"];
    use sharppyrs::thermo;
    assert_close(thermo::wobf(10.0), num(&t["wobf_10"]), 1e-9, "wobf(10)");
    assert_close(thermo::wobf(-30.0), num(&t["wobf_m30"]), 1e-9, "wobf(-30)");
    assert_close(
        thermo::wetlift(850.0, 20.0, 500.0),
        num(&t["wetlift_850_20_500"]),
        1e-6,
        "wetlift",
    );
    let (p2, t2) = thermo::drylift(950.0, 30.0, 20.0);
    assert_close(p2, num(&t["drylift_950_30_20"][0]), 1e-9, "drylift p");
    assert_close(t2, num(&t["drylift_950_30_20"][1]), 1e-9, "drylift t");
    assert_close(
        thermo::virtemp(1000.0, 25.0, 20.0),
        num(&t["virtemp_1000_25_20"]),
        1e-9,
        "virtemp",
    );
    assert_close(
        thermo::wetbulb(900.0, 15.0, 10.0),
        num(&t["wetbulb_900_15_10"]),
        1e-6,
        "wetbulb",
    );
    assert_close(
        thermo::temp_at_mixrat(14.0, 850.0),
        num(&t["temp_at_mixrat_14_850"]),
        1e-9,
        "temp_at_mixrat",
    );
    assert_close(
        thermo::thetae(850.0, 20.0, 15.0),
        num(&t["thetae_850_20_15"]),
        1e-6,
        "thetae",
    );
}

#[test]
fn derived_profile_arrays() {
    let (g, prof) = load();
    assert_eq!(prof.sfc as u64, g["sfc"].as_u64().unwrap());
    assert_eq!(prof.top as u64, g["top"].as_u64().unwrap());
    assert_arr_close(&prof.vtmp, &g["derived"]["vtmp"], 1e-6, "vtmp");
    assert_arr_close(&prof.wetbulb, &g["derived"]["wetbulb"], 1e-4, "wetbulb");
    assert_arr_close(&prof.theta, &g["derived"]["theta"], 1e-6, "theta");
    assert_arr_close(&prof.thetae, &g["derived"]["thetae"], 1e-4, "thetae");
    assert_arr_close(&prof.wvmr, &g["derived"]["wvmr"], 1e-6, "wvmr");
    assert_arr_close(&prof.u, &g["derived"]["u"], 1e-6, "u");
    assert_arr_close(&prof.v, &g["derived"]["v"], 1e-6, "v");
}

#[test]
fn interp_checks() {
    let (g, prof) = load();
    let ps = [900.0, 700.0, 500.0, 300.0, 200.0];
    for (i, p) in ps.iter().enumerate() {
        assert_close(
            sharppyrs::interp::hght(&prof, *p),
            num(&g["interp_hght"][i]),
            1e-6,
            "interp hght",
        );
        assert_close(
            sharppyrs::interp::temp(&prof, *p),
            num(&g["interp_temp"][i]),
            1e-6,
            "interp temp",
        );
        assert_close(
            sharppyrs::interp::vtmp(&prof, *p),
            num(&g["interp_vtmp"][i]),
            1e-6,
            "interp vtmp",
        );
        assert_close(
            sharppyrs::interp::dwpt(&prof, *p),
            num(&g["interp_dwpt"][i]),
            1e-6,
            "interp dwpt",
        );
        let (u, v) = sharppyrs::interp::components(&prof, *p);
        assert_close(u, num(&g["interp_u"][i]), 1e-6, "interp u");
        assert_close(v, num(&g["interp_v"][i]), 1e-6, "interp v");
    }
}

fn check_parcel(prof_pcl: &sharppyrs::Parcel, g: &Value, name: &str, tol: f64) {
    for (field, actual) in [
        ("lclpres", prof_pcl.lclpres),
        ("lclhght", prof_pcl.lclhght),
        ("lfcpres", prof_pcl.lfcpres),
        ("elpres", prof_pcl.elpres),
        ("bplus", prof_pcl.bplus),
        ("bminus", prof_pcl.bminus),
        ("p0c", prof_pcl.p0c),
        ("pm20c", prof_pcl.pm20c),
        ("pm30c", prof_pcl.pm30c),
        ("hght0c", prof_pcl.hght0c),
        ("hghtm20c", prof_pcl.hghtm20c),
        ("hghtm30c", prof_pcl.hghtm30c),
    ] {
        assert_close(actual, num(&g[field]), tol, &format!("{name}.{field}"));
    }
    assert_arr_close(&prof_pcl.ptrace, &g["ptrace"], 1e-6, &format!("{name}.ptrace"));
    assert_arr_close(&prof_pcl.ttrace, &g["ttrace"], 1e-4, &format!("{name}.ttrace"));
}

#[test]
fn parcels_match() {
    let (g, prof) = load();
    check_parcel(&prof.mupcl, &g["mupcl"], "mupcl", 1e-4);
    check_parcel(&prof.sfcpcl, &g["sfcpcl"], "sfcpcl", 1e-4);
    check_parcel(&prof.mlpcl, &g["mlpcl"], "mlpcl", 1e-4);
    check_parcel(&prof.fcstpcl, &g["fcstpcl"], "fcstpcl", 1e-4);
}

#[test]
fn effective_layer_and_kinematics() {
    let (g, prof) = load();
    assert_close(prof.ebottom, num(&g["ebottom"]), 1e-6, "ebottom");
    assert_close(prof.etop, num(&g["etop"]), 1e-6, "etop");
    assert_close(prof.ebotm, num(&g["ebotm"]), 1e-4, "ebotm");
    assert_close(prof.etopm, num(&g["etopm"]), 1e-4, "etopm");
    assert_close(prof.srwind.0, num(&g["srwind"][0]), 1e-4, "srwind rstu");
    assert_close(prof.srwind.1, num(&g["srwind"][1]), 1e-4, "srwind rstv");
    assert_close(prof.srwind.2, num(&g["srwind"][2]), 1e-4, "srwind lstu");
    assert_close(prof.srwind.3, num(&g["srwind"][3]), 1e-4, "srwind lstv");
    assert_close(
        prof.right_esrh,
        num(&g["right_esrh"][0]),
        1e-4,
        "right_esrh",
    );
    assert_close(
        prof.max_lapse_rate_2_6.0,
        num(&g["max_lapse_rate_2_6"][0]),
        1e-4,
        "max lapse rate",
    );
    assert_close(
        prof.max_lapse_rate_2_6.1,
        num(&g["max_lapse_rate_2_6"][1]),
        1e-4,
        "max lapse rate pbot",
    );
    assert_close(
        prof.max_lapse_rate_2_6.2,
        num(&g["max_lapse_rate_2_6"][2]),
        1e-4,
        "max lapse rate ptop",
    );
}

#[test]
fn dcape_and_downdraft_trace() {
    let (g, prof) = load();
    assert_close(prof.dcape, num(&g["dcape"]), 1e-3, "dcape");
    assert_arr_close(&prof.dpcl_ptrace, &g["dpcl_ptrace"], 1e-6, "dpcl_ptrace");
    assert_arr_close(&prof.dpcl_ttrace, &g["dpcl_ttrace"], 1e-3, "dpcl_ttrace");
}

#[test]
fn temp_levels() {
    let (g, prof) = load();
    for (t, key) in [(0.0, "0"), (-10.0, "-10"), (-20.0, "-20"), (-30.0, "-30")] {
        assert_close(
            sharppyrs::params::temp_lvl(&prof, t),
            num(&g["temp_lvl"][key]),
            1e-6,
            &format!("temp_lvl {t}"),
        );
    }
}

#[test]
fn barb_sampling() {
    let (g, prof) = load();
    let bp: Vec<f64> = g["barb_pres"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap_or(f64::NAN))
        .collect();
    for (i, p) in bp.iter().enumerate() {
        let (wdir, wspd) = sharppyrs::interp::vec(&prof, *p);
        assert_close(wdir, num(&g["barb_wdir"][i]), 1e-4, "barb wdir");
        assert_close(wspd, num(&g["barb_wspd"][i]), 1e-4, "barb wspd");
    }
}
