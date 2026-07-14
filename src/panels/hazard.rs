//! "Psbl Haz. Type" box: the vendored watch-box look (`sharppy/viz/watch.py`:
//! centered white title, separator, big centered hazard word) driven by the
//! SHARPpy-Reimagined hazard classifier ported from
//! `sharpmod/sharptab/hazard.py` (`classify` + its pinned threshold decision
//! table), which replaces the legacy `watch_type` logic.
//!
//! Inputs are read off the analyzed profile / derived params — MUCAPE
//! (`prof.mupcl.bplus`), effective SRH (`prof.right_esrh`), EBWD magnitude
//! (`dv.ebwd`), STP (`dv.stp_cin`, falling back to `dv.stp_fixed`), SCP
//! (`dv.right_scp`) and SHIP (`dv.ship`) — and never recomputed. Any missing
//! input degrades to "NONE" instead of a hazard call.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke};

use crate::derived::DerivedParams;
use crate::skewt::SkewTStyle;
use crate::utils::qc;
use crate::Profile;

/// Qt point -> px at the standard 96-dpi factor.
const PT: f64 = 4.0 / 3.0;

/// The `sharpmod.sharptab.hazard.classify` cascade (labels shortened to the
/// SPC-style display words of the box). Returns the word and its color.
fn classify(prof: &Profile, dv: &DerivedParams, style: &SkewTStyle) -> (&'static str, Color32) {
    let none = ("NONE", style.fg_color);

    let mucape = prof.mupcl.bplus;
    let esrh = prof.right_esrh;
    let ebwd = if qc(dv.ebwd.0) && qc(dv.ebwd.1) {
        dv.ebwd.0.hypot(dv.ebwd.1)
    } else {
        f64::NAN
    };
    let stp = if qc(dv.stp_cin) { dv.stp_cin } else { dv.stp_fixed };
    let scp = dv.right_scp;
    let ship = dv.ship;

    // 1. Insufficient data guard -> NONE.
    if !qc(mucape) || !qc(esrh) || !qc(ebwd) || !qc(stp) || !qc(scp) || !qc(ship) {
        return none;
    }
    // 2. No meaningful convection.
    if mucape < 25.0 {
        return none;
    }
    // 3. Significant-tornado environment.
    if stp >= 1.0 && scp >= 1.0 && esrh >= 100.0 && ebwd >= 30.0 {
        return ("TOR", Color32::from_rgb(0xFF, 0x00, 0x00));
    }
    // 4. Organized/rotating storms not meeting the tornado threshold.
    if scp >= 1.0 || ebwd >= 40.0 {
        return ("SUPERCELL", Color32::from_rgb(0xFF, 0xFF, 0x00));
    }
    // 5. Significant-hail environment.
    if ship >= 1.0 {
        return ("HAIL", Color32::from_rgb(0x00, 0xFF, 0xFF));
    }
    // 6. Damaging-wind environment.
    if mucape >= 1000.0 && ebwd >= 30.0 {
        return ("WIND", Color32::from_rgb(0xC8, 0x91, 0x1F));
    }
    // 7. Low-end (marginal) severe potential.
    if mucape >= 500.0 || ebwd >= 20.0 {
        return ("MRGL", Color32::from_rgb(0xE0, 0xA8, 0x00));
    }
    // 8. Otherwise.
    none
}

/// Draw this panel into `rect`.
pub fn draw(painter: &Painter, rect: Rect, prof: &Profile, dv: &DerivedParams, style: &SkewTStyle) {
    let p = painter.with_clip_rect(rect);
    p.rect_filled(rect, 0.0, style.bg_color);

    // Geometry (watch.py initUI): lpad = rpad = tpad = 0, bpad = 20.
    let w = rect.width() as f64;
    let h = rect.height() as f64;
    let bpad = 20.0;
    let brx = w;
    let bry = h - bpad;
    let pad = bry / 100.0;

    let pt = |x: f64, y: f64| Pos2::new(rect.min.x + x as f32, rect.min.y + y as f32);

    // Fonts (font_ratio = 0.0512, like the vendored watch box).
    let title_pt = (h * 0.0512).round() + 5.0;
    let plot_pt = (h * 0.0512).round() + 4.0;
    let title_font = FontId::new((title_pt * PT) as f32, style.font_regular.clone());
    let plot_font = FontId::new((plot_pt * PT) as f32, style.font_regular.clone());
    let fg = style.fg_color;

    // Frame border.
    let border = Stroke::new(2.0, fg);
    p.line_segment([pt(0.0, 0.0), pt(brx, 0.0)], border);
    p.line_segment([pt(brx, 0.0), pt(brx, bry)], border);
    p.line_segment([pt(brx, bry), pt(0.0, bry)], border);
    p.line_segment([pt(0.0, bry), pt(0.0, 0.0)], border);

    // Title bar + separator.
    let title_h = p
        .layout_no_wrap("Psbl Haz. Type".to_string(), title_font.clone(), fg)
        .size()
        .y as f64;
    p.text(
        pt(brx / 2.0, pad * 4.0 + title_h / 2.0),
        Align2::CENTER_CENTER,
        "Psbl Haz. Type",
        title_font,
        fg,
    );
    let sep_y = pad * 4.0 + title_h + 3.0;
    p.line_segment([pt(0.0, sep_y), pt(brx, sep_y)], Stroke::new(1.0, fg));

    // The big hazard word, colored per hazard.
    let (label, color) = classify(prof, dv, style);
    p.text(
        pt(brx / 2.0, bry / 2.0 + title_h / 2.0),
        Align2::CENTER_CENTER,
        label,
        plot_font,
        color,
    );
}
