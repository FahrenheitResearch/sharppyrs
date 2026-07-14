//! "Theta-E v. Pres" inset: port of `sharppy/viz/thetae.py`
//! (`backgroundThetae` + `plotThetae`) with the SHARPpy-Reimagined label
//! layout fixes from `sharpmod/viz/inset_layout.py`: pressure labels
//! left-anchored inside the frame and theta-e labels centered on their tick
//! in the bottom band, skipping any label that would overlap its neighbour.
//!
//! Theta-e (Kelvin) comes from `prof.inner.thetae`; the theta axis range is
//! data-driven (min/max over p > 400 mb, padded by 10 K) like the original.

use egui::{Color32, Align2, FontId, Painter, Pos2, Rect, Stroke};

use crate::derived::DerivedParams;
use crate::skewt::SkewTStyle;
use crate::utils::{int2str, qc};
use crate::Profile;

/// Qt point -> px at the standard 96-dpi factor.
const PT: f64 = 4.0 / 3.0;

/// Draw this panel into `rect`.
pub fn draw(painter: &Painter, rect: Rect, prof: &Profile, dv: &DerivedParams, style: &SkewTStyle) {
    let _ = dv;
    let p = painter.with_clip_rect(rect);
    p.rect_filled(rect, 0.0, style.bg_color);

    // Geometry (initUI): lpad = rpad = tpad = 0, bpad = 20.
    let w = rect.width() as f64;
    let h = rect.height() as f64;
    let bpad = 20.0;
    let brx = w;
    let bry = h - bpad;
    let (pmax, pmin) = (1025.0f64, 400.0f64);

    // Valid (pres, thetae) pairs, surface upward.
    let inner = &prof.inner;
    let mut pairs: Vec<(f64, f64)> = Vec::new();
    for i in 0..inner.pres.len().min(inner.thetae.len()) {
        if qc(inner.pres[i]) && qc(inner.thetae[i]) {
            pairs.push((inner.pres[i], inner.thetae[i]));
        }
    }
    // Theta axis range from the data below 400 mb (defaults like initUI).
    let (mut tmin, mut tmax) = (300.0f64, 360.0f64);
    let mut found = false;
    for &(pr, th) in &pairs {
        if pr > 400.0 {
            if !found {
                tmin = th;
                tmax = th;
                found = true;
            } else {
                tmin = tmin.min(th);
                tmax = tmax.max(th);
            }
        }
    }
    if found {
        tmin -= 10.0;
        tmax += 10.0;
    }

    let pt = |x: f64, y: f64| Pos2::new(rect.min.x + x as f32, rect.min.y + y as f32);
    let pres_to_pix = |pr: f64| bry - ((pmax - pr) / (pmax - pmin)) * bry;
    // Faithful quirk of the original: the theta axis spans [0, bry] in x.
    let theta_to_pix = |t: f64| bry - ((tmax - t) / (tmax - tmin)) * bry;

    let label_pt = (h * 0.0512).round();
    let label_font = FontId::new((label_pt * PT) as f32, style.font_regular.clone());
    let fg = style.fg_color;

    // Frame border.
    let border = Stroke::new(2.0, fg);
    p.line_segment([pt(0.0, 0.0), pt(brx, 0.0)], border);
    p.line_segment([pt(brx, 0.0), pt(brx, bry)], border);
    p.line_segment([pt(brx, bry), pt(0.0, bry)], border);
    p.line_segment([pt(0.0, bry), pt(0.0, 0.0)], border);

    // Title: three lines centered on the (35, 15, 50, 50) box -> (60, 40).
    let lh = p
        .layout_no_wrap("Theta-E".to_string(), label_font.clone(), fg)
        .size()
        .y as f64;
    p.text(pt(60.0, 40.0 - lh), Align2::CENTER_CENTER, "Theta-E", label_font.clone(), fg);
    p.text(pt(60.0, 40.0), Align2::CENTER_CENTER, "v.", label_font.clone(), fg);
    p.text(pt(60.0, 40.0 + lh), Align2::CENTER_CENTER, "Pres", label_font.clone(), fg);

    // Isobar ticks + labels (patched: left-anchored inside the frame).
    let tick = Stroke::new(1.0, fg);
    for pres_lbl in [1000.0, 900.0, 800.0, 700.0, 600.0, 500.0] {
        let y1 = pres_to_pix(pres_lbl);
        p.line_segment([pt(0.0, y1), pt(5.0, y1)], tick);
        p.line_segment([pt(brx - 5.0, y1), pt(brx, y1)], tick);
        p.text(
            pt(7.0, y1),
            Align2::LEFT_CENTER,
            int2str(pres_lbl),
            label_font.clone(),
            fg,
        );
    }

    // Theta-E ticks every 10 K + non-overlapping labels in the bottom band.
    let mut last_right = f64::NEG_INFINITY;
    let mut t = 200.0f64;
    while t < 400.0 {
        let x1 = theta_to_pix(t);
        p.line_segment([pt(x1, 0.0), pt(x1, 5.0)], tick);
        p.line_segment([pt(x1, bry - 5.0), pt(x1, bry)], tick);
        let text = int2str(t);
        let tw = p
            .layout_no_wrap(text.clone(), label_font.clone(), fg)
            .size()
            .x as f64;
        let box_w = (tw + 8.0).max(30.0);
        let left = x1 - box_w / 2.0;
        let right = left + box_w;
        if left >= 0.0 && right <= brx && left >= last_right + 3.0 {
            p.text(pt(x1, bry + 3.0), Align2::CENTER_TOP, text, label_font.clone(), fg);
            last_right = right;
        }
        t += 10.0;
    }

    // Theta-E profile trace, 2 px, lowest 400+ mb only, colored per segment
    // by theta-e value (Pivotal-style banded table).
    for i in 0..pairs.len().saturating_sub(1) {
        if pairs[i].0 > 400.0 {
            let (p1, t1) = pairs[i];
            let (p2, t2) = pairs[i + 1];
            let stroke = Stroke::new(2.0, thetae_color((t1 + t2) / 2.0));
            p.line_segment(
                [
                    pt(theta_to_pix(t1), pres_to_pix(p1)),
                    pt(theta_to_pix(t2), pres_to_pix(p2)),
                ],
                stroke,
            );
        }
    }
}

/// Pivotal-style theta-e color bands (K): 10-K tiers from 360+ down to 230,
/// deep greens on top, turning white near 320 and descending through greys
/// below 310.
fn thetae_color(te_k: f64) -> Color32 {
    const BANDS: [(f64, Color32); 14] = [
        (360.0, Color32::from_rgb(0x00, 0x5C, 0x24)),
        (350.0, Color32::from_rgb(0x14, 0x7A, 0x32)),
        (340.0, Color32::from_rgb(0x2E, 0x9E, 0x41)),
        (330.0, Color32::from_rgb(0x66, 0xC1, 0x6C)),
        (320.0, Color32::from_rgb(0xFF, 0xFF, 0xFF)),
        (310.0, Color32::from_rgb(0xD4, 0xD4, 0xD4)),
        (300.0, Color32::from_rgb(0xC0, 0xC0, 0xC0)),
        (290.0, Color32::from_rgb(0xAC, 0xAC, 0xAC)),
        (280.0, Color32::from_rgb(0x99, 0x99, 0x99)),
        (270.0, Color32::from_rgb(0x87, 0x87, 0x87)),
        (260.0, Color32::from_rgb(0x76, 0x76, 0x76)),
        (250.0, Color32::from_rgb(0x67, 0x67, 0x67)),
        (240.0, Color32::from_rgb(0x59, 0x59, 0x59)),
        (230.0, Color32::from_rgb(0x4D, 0x4D, 0x4D)),
    ];
    for (lo, c) in BANDS {
        if te_k >= lo {
            return c;
        }
    }
    BANDS[BANDS.len() - 1].1
}
