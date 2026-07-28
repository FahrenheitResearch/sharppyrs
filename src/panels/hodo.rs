//! The SPC-style hodograph panel: a faithful egui port of
//! `sharppy.viz.hodo` (`backgroundHodo.plotBackground` + `plotHodo.plotData`,
//! SHARPpy 1.4.0a5) plus the SHARPpy-Reimagined overlay passes:
//! the 200-kt zoom (`render._install_hodo_zoom`, `HODO_ZOOM_KTS`), the
//! 0-500 m magenta trace band (`render._install_hodo_0500`) and the top-left
//! locator inset (`sharpmod.viz.hodo_locator`, drawn here from an embedded
//! Natural Earth basemap instead of the live TIGERweb county query).
//! Interactivity (dragging the trace / storm-motion vectors, cursor readouts,
//! boundary cursor) is intentionally not ported.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

use sharprs::profile::{comp2vec, vec2comp};

use crate::derived::DerivedParams;
use crate::skewt::SkewTStyle;
use crate::utils::{int2str, qc};
use crate::Profile;

use super::hodo_map_data;

// SPC default colors of the original (config defaults hardcoded, like the
// Python widget does for everything the preferences dialog cannot change).
const ISOTACH_COLOR: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
/// Height-band trace colors: 0-500 m (SHARPpy-Reimagined `HODO_0_500_COLOR`),
/// then the config defaults for 0-3 / 3-6 / 6-9 / 9-12 km.
const TRACE_COLORS: [Color32; 5] = [
    Color32::from_rgb(0xFF, 0x00, 0xFF),
    Color32::from_rgb(0xFF, 0x00, 0x00),
    Color32::from_rgb(0x00, 0xFF, 0x00),
    Color32::from_rgb(0xFF, 0xFF, 0x00),
    Color32::from_rgb(0x00, 0xFF, 0xFF),
];
const EFF_INFLOW_COLOR: Color32 = Color32::from_rgb(0x00, 0xFF, 0xFF);
const CRIT_TEXT_COLOR: Color32 = Color32::from_rgb(0x00, 0xFF, 0xFF);
const CRIT_LINE_COLOR: Color32 = Color32::from_rgb(0xFF, 0x00, 0xFF);
const CORFIDI_COLOR: Color32 = Color32::from_rgb(0x00, 0xBF, 0xFF);
const MEAN_LCL_EL_COLOR: Color32 = Color32::from_rgb(0xB8, 0x86, 0x0B);
// Locator inset palette (sharpmod.viz.hodo_locator module constants).
const MAP_FILL: Color32 = Color32::from_rgb(0x05, 0x09, 0x0B);
const MAP_POINT_COLOR: Color32 = Color32::from_rgb(0xFF, 0xDA, 0x00);

/// Geometry + transforms (port of `backgroundHodo.initUI` with the
/// SHARPpy-Reimagined zoom, widened ~12% per field feedback).
/// Default hodograph window (kts across); SHARPpy-Reimagined uses 200,
/// widened 25% per field feedback. Scroll-zoomable in `SoundingView`.
pub(crate) const DEFAULT_ZOOM_KTS: f64 = 250.0;

struct Geom {
    rect: Rect,
    wid: f64,
    hgt: f64,
    centerx: f64,
    centery: f64,
    /// Pixels per knot.
    scale: f64,
    /// The window width `scale` was built from: the caller's `zoom_kts` after
    /// clamping to the interactive range, so `geometry` can report the number
    /// actually drawn rather than the number asked for.
    zoom_kts: f64,
}

impl Geom {
    /// The drawn area: the largest centered square that fits inside `cell`.
    ///
    /// `scale` is a single isotropic px-per-kt shared by both axes (see
    /// `uv_to_pix`), so in a cell wider than it is tall the vertical axis
    /// reaches fewer knots than the horizontal one -- at the rusty-weather
    /// layout that is +/-89 kt against +/-125 kt, which reads as a bug even
    /// though the geometry is correct. Squaring the drawn area gives both axes
    /// the same range. Stretching to fill the cell instead would turn the
    /// isotach rings into ellipses and misstate every shear vector's magnitude
    /// and the critical angle, so the leftover width stays background
    /// (`draw_zoomed` fills the whole cell before drawing).
    fn plot_rect(cell: Rect) -> Rect {
        let side = cell.width().min(cell.height());
        Rect::from_center_size(cell.center(), Vec2::splat(side))
    }

    fn with_zoom(cell: Rect, zoom_kts: f64) -> Geom {
        let rect = Geom::plot_rect(cell);
        let wid = rect.width() as f64;
        let hgt = rect.height() as f64;
        let hodomag = zoom_kts.clamp(80.0, 500.0);
        Geom {
            rect,
            wid,
            hgt,
            centerx: wid / 2.0,
            centery: hgt / 2.0,
            scale: wid / hodomag,
            zoom_kts: hodomag,
        }
    }

    /// (u, v) kts -> widget-local px.
    fn uv_to_pix(&self, u: f64, v: f64) -> (f64, f64) {
        (
            self.centerx + u * self.scale,
            self.centery - v * self.scale,
        )
    }

    /// Widget-local -> screen position.
    fn pt(&self, x: f64, y: f64) -> Pos2 {
        Pos2::new(self.rect.min.x + x as f32, self.rect.min.y + y as f32)
    }
}

/// The hodograph plot geometry, enough for a client holding only an image to
/// map a pixel back to a wind. Units are KNOTS throughout:
///
/// ```text
/// u = (x - center.x) / px_per_kt;  v = (center.y - y) / px_per_kt
/// spd = hypot(u, v);  dir = (atan2(u, v).to_degrees() + 180) rem_euclid 360
/// ```
///
/// (that `dir` is `sharprs::profile::comp2vec`.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HodoGeometry {
    /// The drawn area: the centered square inside the cell, NOT the cell — the
    /// px-per-kt scale is isotropic and comes from the square's width, so a
    /// readout referred to the cell would be offset (see [`Geom::plot_rect`]).
    pub plot: Rect,
    /// Screen position of the (u, v) = (0, 0) origin.
    pub center: Pos2,
    /// Pixels per knot, the same on both axes.
    pub px_per_kt: f64,
    /// Window width in knots actually used: the requested zoom clamped to the
    /// interactive 80–500 kt range.
    pub zoom_kts: f64,
}

/// The geometry of a hodograph drawn into `cell` at `zoom_kts` — the same
/// numbers [`draw_zoomed`] plots with, for hosts that render headless and must
/// explain a pixel.
pub fn geometry(cell: Rect, zoom_kts: f64) -> HodoGeometry {
    let g = Geom::with_zoom(cell, zoom_kts);
    HodoGeometry {
        plot: g.rect,
        center: g.pt(g.centerx, g.centery),
        px_per_kt: g.scale,
        zoom_kts: g.zoom_kts,
    }
}

/// Fonts sized from the widget height like the Qt original (pt -> px at the
/// standard 96-dpi factor 4/3). Hodo fsize = 7 (the > 75-dpi branch).
struct Fonts {
    label: FontId,
    critical: FontId,
    critical_height: f64,
}

impl Fonts {
    fn new(hgt: f64, style: &SkewTStyle) -> Fonts {
        const PT: f64 = 4.0 / 3.0;
        let fsize = 7.0;
        let label_pt = fsize + hgt * 0.0045;
        let critical_pt = fsize + 2.0 + hgt * 0.0045;
        Fonts {
            label: style.bold_font((label_pt * PT) as f32),
            critical: style.bold_font((critical_pt * PT) as f32),
            // xHeight() + 5 + hgt*0.0045, approximated like skewt::Fonts.
            critical_height: critical_pt * PT * 0.5 + 5.0 + hgt * 0.0045,
        }
    }
}

#[cfg(test)]
mod typography_tests {
    use super::*;
    use crate::skewt::SoundingFontPreset;

    #[test]
    fn shared_typography_reaches_hodograph_labels_without_geometry_scaling() {
        let base = Fonts::new(440.0, &SkewTStyle::default());
        let style = SkewTStyle::default()
            .with_font_preset(SoundingFontPreset::TechnicalMonospace)
            .with_text_scale(1.4);
        let scaled = Fonts::new(440.0, &style);

        assert!((scaled.label.size - base.label.size * 1.4).abs() < 0.001);
        assert_eq!(scaled.label.family, egui::FontFamily::Monospace);
        assert_eq!(scaled.critical_height, base.critical_height);
    }
}

#[cfg(test)]
mod geometry_tests {
    use super::*;

    #[test]
    fn the_square_plot_area_is_centered_in_a_wide_cell() {
        let cell = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(400.0, 200.0));
        let plot = Geom::plot_rect(cell);

        assert_eq!(plot.width(), 200.0);
        assert_eq!(plot.height(), 200.0);
        assert!((plot.center().x - cell.center().x).abs() < 1.0e-4);
        assert!((plot.center().y - cell.center().y).abs() < 1.0e-4);
    }

    #[test]
    fn both_axes_reach_the_same_knots_and_label_ninety() {
        // The rusty-weather diagnostic board hands the hodograph a cell of
        // roughly 576x409 pt. Before squaring, the shared isotropic scale left
        // the vertical axis at +/-89 kt against the horizontal's +/-125, so the
        // 90 ring's labels fell a couple of pixels outside the clip rect and
        // the axis appeared to stop at 80.
        let cell = Rect::from_min_size(Pos2::new(1350.7, 45.0), Vec2::new(575.6, 409.4));
        let g = Geom::with_zoom(cell, 195.0);

        assert!((g.wid - g.hgt).abs() < 1.0e-4, "plot area must be square");

        let horizontal_kt = g.centerx / g.scale;
        let vertical_kt = g.centery / g.scale;
        assert!(
            (horizontal_kt - vertical_kt).abs() < 1.0e-4,
            "axes must span equal knots: {horizontal_kt} vs {vertical_kt}"
        );

        // 90 is the outermost LABELED ring in all four directions: its labels fit
        // whole, 100's do not. Rings past 90 still draw, unlabeled, out to the
        // corners -- what the cap removes is the half-clipped number on the frame.
        let fonts = Fonts::new(g.hgt, &SkewTStyle::default());
        let reach = g.centerx.min(g.centery) - label_slack(&fonts);
        assert!(90.0 * g.scale <= reach, "90 must be labeled");
        assert!(100.0 * g.scale > reach, "100 must not be labeled");
    }
}

/// Draw this panel into `rect`.
pub fn draw(painter: &Painter, rect: Rect, prof: &Profile, dv: &DerivedParams, style: &SkewTStyle) {
    draw_zoomed(painter, rect, prof, dv, style, DEFAULT_ZOOM_KTS)
}

/// `draw` with an explicit hodograph window width in knots.
pub(crate) fn draw_zoomed(
    painter: &Painter,
    rect: Rect,
    prof: &Profile,
    dv: &DerivedParams,
    style: &SkewTStyle,
    zoom_kts: f64,
) {
    if rect.width() < 50.0 || rect.height() < 50.0 {
        return;
    }
    let p = painter.with_clip_rect(rect);
    // Background covers the whole cell, so the letterbox gutters either side of
    // the square plot area read as panel background rather than as a hole.
    p.rect_filled(rect, 0.0, style.bg_color);

    let g = Geom::with_zoom(rect, zoom_kts);
    // Clip to the square: the isotach rings run out to the corner distance, so
    // without this the arcs beyond the square's edges spill into the gutters
    // and the frame stops reading as a frame.
    let p = p.with_clip_rect(g.rect);
    let fonts = Fonts::new(g.hgt, style);

    draw_background(&p, &g, &fonts, style);
    draw_data(&p, &g, &fonts, prof, dv, style);
    // The locator map now lives in its own panel (see panels::locator);
    // keep the hodograph corner clear like upstream SHARPpy.
    let _ = draw_locator;
}

// ----------------------------------------------------------------------
// Background pass (port of backgroundHodo.plotBackground)
// ----------------------------------------------------------------------
fn draw_background(p: &Painter, g: &Geom, fonts: &Fonts, style: &SkewTStyle) {
    // Speed rings out to the corner, every 10 kt -- but only LABEL the rings
    // whose labels fit whole. The four labels sit on the axes at +/- the ring
    // radius (see `draw_ring`), so a ring that itself fits can still put its
    // label across the frame edge, and the clip rect then draws a sliver of a
    // half number riding the border. There is no zoom that avoids that by luck:
    // the +2.5 nudge is asymmetric and the horizontal labels are wider than the
    // vertical ones are tall, so the "fits" windows for the two directions do
    // not overlap. Capping the labels fixes it for any cell shape and any zoom.
    let max_uv = (g.centerx.hypot(g.centery) / g.scale) as i64;
    let label_reach = g.centerx.min(g.centery) - label_slack(fonts);
    let mut spd = 10i64;
    while spd <= max_uv + 10 {
        let r = spd as f64 * g.scale;
        draw_ring(p, g, fonts, style, spd as f64, r <= label_reach);
        spd += 10;
    }
    // Axes + frame (fg white, 2 px).
    let stroke = Stroke::new(2.0, style.fg_color);
    p.line_segment([g.pt(g.centerx, 0.0), g.pt(g.centerx, g.hgt)], stroke);
    p.line_segment([g.pt(0.0, g.centery), g.pt(g.wid, g.centery)], stroke);
    p.line_segment([g.pt(0.0, 0.0), g.pt(g.wid, 0.0)], stroke);
    p.line_segment([g.pt(g.wid, 0.0), g.pt(g.wid, g.hgt)], stroke);
    p.line_segment([g.pt(g.wid, g.hgt), g.pt(0.0, g.hgt)], stroke);
    p.line_segment([g.pt(0.0, g.hgt), g.pt(0.0, 0.0)], stroke);
}

/// How far short of the frame a ring's radius must stop for its labels to fit
/// whole: the original's +2.5 px nudge plus half a label. The widest label is
/// three digits, and a horizontal label clips on width where a vertical one
/// clips on height, so half-WIDTH is the binding case and covers both.
fn label_slack(fonts: &Fonts) -> f64 {
    const NUDGE: f64 = 2.5;
    NUDGE + f64::from(fonts.label.size) * 0.9
}

/// Port of `backgroundHodo.draw_ring`: dashed isotach circle, plus the four
/// speed labels sitting on the axes when `label` (see `draw_background`).
fn draw_ring(p: &Painter, g: &Geom, fonts: &Fonts, style: &SkewTStyle, spd: f64, label: bool) {
    let r = spd * g.scale;
    // Qt drawEllipse with a DashLine pen (dash 4, gap 2 at width 1).
    let n = (r as usize).clamp(64, 512);
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let a = std::f64::consts::TAU * i as f64 / n as f64;
        pts.push(g.pt(g.centerx + r * a.cos(), g.centery + r * a.sin()));
    }
    p.extend(Shape::dashed_line(&pts, Stroke::new(1.0, ISOTACH_COLOR), 4.0, 2.0));

    if !label {
        return;
    }

    // Labels: AlignCenter in 15x15 rects offset 5 px from the axes
    // (centers at +/- ring radius, nudged +12.5 / +2.5 like the original).
    let text = int2str(spd);
    let centers = [
        (g.centerx + 12.5, g.centery - r + 2.5),
        (g.centerx + 12.5, g.centery + r + 2.5),
        (g.centerx - r + 2.5, g.centery + 12.5),
        (g.centerx + r + 2.5, g.centery + 12.5),
    ];
    for (x, y) in centers {
        p.text(
            g.pt(x, y),
            Align2::CENTER_CENTER,
            text.clone(),
            fonts.label.clone(),
            style.fg_color,
        );
    }
}

// ----------------------------------------------------------------------
// Data pass (port of plotHodo.plotData)
// ----------------------------------------------------------------------
fn draw_data(
    p: &Painter,
    g: &Geom,
    fonts: &Fonts,
    prof: &Profile,
    dv: &DerivedParams,
    style: &SkewTStyle,
) {
    // ONLY DRAW A HODOGRAPH IF THERE'S WIND DATA (prof.wdir.count() > 1).
    if !draw_hodo_trace(p, g, prof) {
        return;
    }
    // Every marker (and the frame-anchored critical-angle readout) is drawn
    // first and the marker labels last, as one group: the group layout has to
    // measure every anchor before it can put any text down (see
    // `draw_marker_labels`).
    let mut labels = Vec::new();
    draw_smv(p, g, prof, style, &mut labels);
    draw_corfidi(p, g, dv, &mut labels);
    draw_lcl_to_el_mw(p, g, dv, &mut labels);
    let fixed: Vec<Rect> = draw_critical_angle(p, g, fonts, prof, dv, style)
        .into_iter()
        .collect();
    draw_marker_labels(p, g, fonts, style, &labels, &fixed);
}

/// Linear interpolation of `f` at `zt` over ascending `z`
/// (port of `interp.generic_interp_hght` / np.interp, clamped at the ends).
fn interp_z(z: &[f64], f: &[f64], zt: f64) -> f64 {
    if zt <= z[0] {
        return f[0];
    }
    for i in 1..z.len() {
        if z[i] >= zt {
            let (z0, z1) = (z[i - 1], z[i]);
            if z1 == z0 {
                return f[i];
            }
            return f[i - 1] + (zt - z0) / (z1 - z0) * (f[i] - f[i - 1]);
        }
    }
    *f.last().unwrap()
}

/// The height-colored trace (SHARPpy-Reimagined `draw_hodo` override: band
/// edges at 0/0.5/3/6/9/12 km AGL). Returns false when there is no wind data.
fn draw_hodo_trace(p: &Painter, g: &Geom, prof: &Profile) -> bool {
    let inner = &prof.inner;
    let n = inner.u.len().min(inner.v.len()).min(inner.hght.len());
    let mut z: Vec<f64> = Vec::new();
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    for i in 0..n {
        if inner.u[i].is_finite() && inner.v[i].is_finite() && inner.hght[i].is_finite() {
            let (x, y) = g.uv_to_pix(inner.u[i], inner.v[i]);
            z.push(inner.to_agl(inner.hght[i]));
            xs.push(x);
            ys.push(y);
        }
    }
    if z.len() < 2 {
        return false;
    }
    let zmin = z.iter().cloned().fold(f64::INFINITY, f64::min);
    let zmax = z.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let seg_bnds: Vec<f64> = [0.0, 500.0, 3000.0, 6000.0, 9000.0, 12000.0]
        .iter()
        .map(|b: &f64| b.max(zmin))
        .collect();
    let nseg = seg_bnds.iter().filter(|b| **b <= zmax).count();
    let seg_pos: Vec<(f64, f64)> = seg_bnds[..nseg]
        .iter()
        .map(|b| (interp_z(&z, &xs, *b), interp_z(&z, &ys, *b)))
        .collect();
    // np.searchsorted (left): first index with z[i] >= bnd.
    let seg_idx: Vec<usize> = seg_bnds
        .iter()
        .map(|b| z.partition_point(|v| v < b))
        .collect();

    for idx in 0..nseg.saturating_sub(1) {
        let mut pts = vec![g.pt(seg_pos[idx].0, seg_pos[idx].1)];
        for zi in seg_idx[idx]..seg_idx[idx + 1] {
            pts.push(g.pt(xs[zi], ys[zi]));
        }
        pts.push(g.pt(seg_pos[idx + 1].0, seg_pos[idx + 1].1));
        p.add(Shape::line(pts, Stroke::new(2.0, TRACE_COLORS[idx])));
    }
    // Data top below 12 km AGL: finish the last band with the raw tail.
    if zmax < 12000.0 {
        let idx = nseg - 1;
        let mut pts = vec![g.pt(seg_pos[idx].0, seg_pos[idx].1)];
        for zi in seg_idx[idx]..z.len() {
            pts.push(g.pt(xs[zi], ys[zi]));
        }
        if pts.len() >= 2 {
            p.add(Shape::line(pts, Stroke::new(2.0, TRACE_COLORS[idx])));
        }
    }
    true
}

/// Port of `plotHodo.drawSMV`: Bunkers +'s and circles, effective-inflow
/// lines to the right mover, and the "259/48 RM" / "237/68 LM" labels.
fn draw_smv(
    p: &Painter,
    g: &Geom,
    prof: &Profile,
    style: &SkewTStyle,
    labels: &mut Vec<MarkerLabel>,
) {
    let (rstu, rstv, lstu, lstv) = prof.srwind;
    if !qc(rstu) || !qc(lstu) {
        return;
    }
    let stroke = Stroke::new(1.0, style.fg_color);
    // +'s at the Bunkers vector locations (prof.bunkers == prof.srwind here).
    let (rx, ry) = g.uv_to_pix(rstu, rstv);
    let (lx, ly) = g.uv_to_pix(lstu, lstv);
    for (x, y) in [(rx, ry), (lx, ly)] {
        p.line_segment([g.pt(x - 2.0, y), g.pt(x + 2.0, y)], stroke);
        p.line_segment([g.pt(x, y - 2.0), g.pt(x, y + 2.0)], stroke);
    }
    // Circles around the storm motion vectors.
    p.circle_stroke(g.pt(rx, ry), 5.0, stroke);
    p.circle_stroke(g.pt(lx, ly), 5.0, stroke);

    // Effective inflow layer markers: lines from the right mover to the
    // interpolated wind at the layer bottom/top.
    if qc(prof.etop) && qc(prof.ebottom) {
        let (utop, vtop) = prof.inner.interp_wind(prof.etop);
        let (ubot, vbot) = prof.inner.interp_wind(prof.ebottom);
        let eff = Stroke::new(1.0, EFF_INFLOW_COLOR);
        if qc(ubot) && qc(vbot) {
            let (bx, by) = g.uv_to_pix(ubot, vbot);
            p.line_segment([g.pt(rx, ry), g.pt(bx, by)], eff);
        }
        if qc(utop) && qc(vtop) {
            let (tx, ty) = g.uv_to_pix(utop, vtop);
            p.line_segment([g.pt(rx, ry), g.pt(tx, ty)], eff);
        }
    }

    // Labels in 55x12 rects offset (+2, +5) from the markers.
    let (rm_dir, rm_spd) = comp2vec(rstu, rstv);
    let (lm_dir, lm_spd) = comp2vec(lstu, lstv);
    labels.push(MarkerLabel {
        marker: g.pt(rx, ry),
        center: g.pt(rx + 2.0 + 27.5, ry + 5.0 + 6.0),
        text: format!("{}/{} RM", int2str(rm_dir), int2str(rm_spd)),
        color: style.fg_color,
    });
    labels.push(MarkerLabel {
        marker: g.pt(lx, ly),
        center: g.pt(lx + 2.0 + 27.5, ly + 5.0 + 6.0),
        text: format!("{}/{} LM", int2str(lm_dir), int2str(lm_spd)),
        color: style.fg_color,
    });
}

/// Port of `plotHodo.drawCorfidi`: upshear/downshear circles + labels.
fn draw_corfidi(p: &Painter, g: &Geom, dv: &DerivedParams, labels: &mut Vec<MarkerLabel>) {
    let (up_u, up_v) = dv.corfidi_up;
    let (dn_u, dn_v) = dv.corfidi_dn;
    if !qc(up_u) || !qc(up_v) || !qc(dn_u) || !qc(dn_v) {
        return;
    }
    let stroke = Stroke::new(1.0, CORFIDI_COLOR);
    let (ux, uy) = g.uv_to_pix(up_u, up_v);
    let (dx, dy) = g.uv_to_pix(dn_u, dn_v);
    p.circle_stroke(g.pt(ux, uy), 3.0, stroke);
    p.circle_stroke(g.pt(dx, dy), 3.0, stroke);

    // Labels in 60x10 rects offset (+1, +3).
    let (up_dir, up_spd) = comp2vec(up_u, up_v);
    let (dn_dir, dn_spd) = comp2vec(dn_u, dn_v);
    labels.push(MarkerLabel {
        marker: g.pt(ux, uy),
        center: g.pt(ux + 1.0 + 30.0, uy + 3.0 + 5.0),
        text: format!("UP={}/{}", int2str(up_dir), int2str(up_spd)),
        color: CORFIDI_COLOR,
    });
    labels.push(MarkerLabel {
        marker: g.pt(dx, dy),
        center: g.pt(dx + 1.0 + 30.0, dy + 3.0 + 5.0),
        text: format!("DN={}/{}", int2str(dn_dir), int2str(dn_spd)),
        color: CORFIDI_COLOR,
    });
}

/// Port of `plotHodo.drawLCLtoEL_MW`: the LCL-EL mean wind square + label.
fn draw_lcl_to_el_mw(p: &Painter, g: &Geom, dv: &DerivedParams, labels: &mut Vec<MarkerLabel>) {
    let (mw_dir, mw_spd) = dv.mean_lcl_el;
    if !qc(mw_dir) || !qc(mw_spd) {
        return;
    }
    let (mu, mv) = vec2comp(mw_dir, mw_spd);
    let (mx, my) = g.uv_to_pix(mu, mv);
    // Qt: drawRect(mean_u - 4, mean_v + 4, 8, 8) with a 2 px pen.
    p.rect_stroke(
        Rect::from_min_size(g.pt(mx - 4.0, my + 4.0), Vec2::new(8.0, 8.0)),
        0.0,
        Stroke::new(2.0, MEAN_LCL_EL_COLOR),
        StrokeKind::Middle,
    );
    // Label in a 40x12 rect offset (+1, +5).
    labels.push(MarkerLabel {
        marker: g.pt(mx, my),
        center: g.pt(mx + 1.0 + 20.0, my + 5.0 + 6.0),
        text: format!("{}/{}", int2str(mw_dir), int2str(mw_spd)),
        color: MEAN_LCL_EL_COLOR,
    });
}

/// Port of `plotHodo.drawCriticalAngle`: the sfc -> 500 m AGL line plus the
/// "Critical Angle = 56 deg" readout at the bottom-left of the frame. Returns
/// the readout's blanked box when it is drawn, so the marker labels can treat
/// it as occupied space (its position is fixed to the frame, theirs is not).
fn draw_critical_angle(
    p: &Painter,
    g: &Geom,
    fonts: &Fonts,
    prof: &Profile,
    dv: &DerivedParams,
    style: &SkewTStyle,
) -> Option<Rect> {
    let inner = &prof.inner;
    let sfc_pres = inner.pres[inner.sfc];
    let pres_500m = inner.pres_at_height(inner.to_msl(500.0));
    let (sfc_u, sfc_v) = inner.interp_wind(sfc_pres);
    let (u500, v500) = if pres_500m.is_finite() {
        inner.interp_wind(pres_500m)
    } else {
        (f64::NAN, f64::NAN)
    };
    // Only drawn when the effective inflow layer is surface based.
    if !qc(prof.etop)
        || !qc(prof.ebottom)
        || prof.ebottom != sfc_pres
        || !qc(sfc_u)
        || !qc(sfc_v)
        || !qc(u500)
        || !qc(v500)
    {
        return None;
    }
    let (sx, sy) = g.uv_to_pix(sfc_u, sfc_v);
    let (x5, y5) = g.uv_to_pix(u500, v500);
    p.line_segment([g.pt(sx, sy), g.pt(x5, y5)], Stroke::new(1.0, CRIT_LINE_COLOR));

    let (rstu, _, lstu, _) = prof.srwind;
    if !qc(rstu) || !qc(lstu) || !qc(dv.right_critical_angle) {
        return None;
    }
    // Half-alpha bg blank behind the text (setBlackPen), then the readout.
    let bg = style.bg_color;
    let blank = Rect::from_min_size(
        g.pt(15.0, g.hgt - 36.0),
        Vec2::new(100.0, (fonts.critical_height + 5.0) as f32),
    );
    p.rect_filled(
        blank,
        0.0,
        Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 128),
    );
    p.text(
        g.pt(15.0, g.hgt - 36.0),
        Align2::LEFT_TOP,
        format!("Critical Angle = {}\u{00B0}", int2str(dv.right_critical_angle)),
        fonts.critical.clone(),
        CRIT_TEXT_COLOR,
    );
    Some(blank)
}

// ----------------------------------------------------------------------
// Marker labels
//
// `plotData` places each readout at a fixed pixel offset from its own marker,
// which only works while the markers are apart. In weak flow the Bunkers pair,
// both Corfidi nodes and the LCL-EL mean wind all collapse toward the origin
// and the five labels print on top of each other -- observed inside a ~40 px
// circle, and unreadable at every zoom because the offsets are in pixels, so no
// amount of scaling separates them.
//
// The labels are therefore laid out as a group once every marker is down. When
// the measured rects touch nothing the group is drawn at exactly the original
// offsets, so a plate whose markers are already separated is pixel-for-pixel
// what it was. Only a tangled group is spread, by sliding labels along y: they
// are ~4x wider than tall, so y is the axis of least travel, and sliding one
// axis only keeps each label horizontally where its marker put it. Order is the
// order the data pass queues them (RM, LM, UP, DN, mean wind) so the same
// profile always lays out the same way, and a spread group gets the opaque
// blank `cursor_marker` uses, because a displaced label no longer has the clear
// space beside its own marker to itself and would otherwise read through the
// trace and the ring labels.
// ----------------------------------------------------------------------

/// A marker readout queued for the group layout pass.
struct MarkerLabel {
    /// Center of the drawn marker. The label may cover its own marker -- the
    /// "240/19 RM" beside its + is the whole SHARPpy idiom -- but covering one
    /// of the others would hide data.
    marker: Pos2,
    /// Where `plotData` centers the text. Kept verbatim as the first choice.
    center: Pos2,
    text: String,
    color: Color32,
}

/// Half-size of a marker's keep-out box: the Bunkers circles are r=5, the
/// mean-wind square +/-4 with a 2 px pen.
const MARKER_KEEP_OUT: f32 = 6.0;
/// Clear space kept between two spread labels, and between a label and the
/// frame -- the panel is clipped to the frame, so a label pushed out is lost.
const LABEL_GAP: f32 = 3.0;
/// Padding of the blank drawn behind a spread label (`cursor_marker`'s).
const BLANK_PAD: Vec2 = Vec2::new(2.0, 1.0);

fn draw_marker_labels(
    p: &Painter,
    g: &Geom,
    fonts: &Fonts,
    style: &SkewTStyle,
    labels: &[MarkerLabel],
    fixed: &[Rect],
) {
    // A marker outside the frame is clipped away; pulling its label into view
    // would leave a readout pointing at nothing.
    let queued: Vec<&MarkerLabel> = labels
        .iter()
        .filter(|l| g.rect.contains(l.marker))
        .collect();
    let galleys: Vec<_> = queued
        .iter()
        .map(|l| p.layout_no_wrap(l.text.clone(), fonts.label.clone(), l.color))
        .collect();
    // `Painter::text` with Align2::CENTER_CENTER lands the galley at exactly
    // this rect's min, which is what keeps the untangled case unchanged.
    let wanted: Vec<Rect> = queued
        .iter()
        .zip(&galleys)
        .map(|(l, galley)| Rect::from_center_size(l.center, galley.size()))
        .collect();

    let tangled = labels_are_tangled(&wanted, fixed);
    let placed = if tangled {
        let markers: Vec<Rect> = queued
            .iter()
            .map(|l| Rect::from_center_size(l.marker, Vec2::splat(2.0 * MARKER_KEEP_OUT)))
            .collect();
        let spread = spread_labels(&wanted, &markers, fixed, g.rect.shrink(LABEL_GAP));
        // All the blanks before any of the text: the boxes never overlap, but
        // drawing them interleaved would still let a later blank clip the
        // descenders of an earlier label.
        for r in &spread {
            p.rect_filled(r.expand2(BLANK_PAD), 0.0, style.bg_color);
        }
        spread
    } else {
        wanted
    };

    for ((l, galley), r) in queued.iter().zip(galleys).zip(&placed) {
        p.galley(r.min, galley, l.color);
    }
}

/// Whether any label in the group touches another one or a frame-anchored
/// readout, i.e. whether the group needs spreading at all.
fn labels_are_tangled(wanted: &[Rect], fixed: &[Rect]) -> bool {
    wanted.iter().enumerate().any(|(i, a)| {
        fixed.iter().any(|f| f.intersects(*a)) || wanted[i + 1..].iter().any(|b| a.intersects(*b))
    })
}

/// Slide each label along y until it clears the ones already placed, every
/// marker but its own, and `fixed`, staying inside `safe`. One rect per
/// `wanted`, in order and at the same size, so the result is a function of the
/// data alone. `markers` is parallel to `wanted`.
fn spread_labels(wanted: &[Rect], markers: &[Rect], fixed: &[Rect], safe: Rect) -> Vec<Rect> {
    let mut placed: Vec<Rect> = Vec::with_capacity(wanted.len());
    for (i, want) in wanted.iter().enumerate() {
        let mut blocked = placed.clone();
        blocked.extend_from_slice(fixed);
        for (j, m) in markers.iter().enumerate() {
            if j != i {
                blocked.push(*m);
            }
        }
        let start = clamp_inside(*want, safe);
        let down = slide_clear(start, &blocked, safe, 1.0);
        let up = slide_clear(start, &blocked, safe, -1.0);
        placed.push(match (down, up) {
            // Least travel, downward on a tie -- the original offsets are
            // downward, so a two-label group keeps reading the familiar way.
            (Some(d), Some(u)) => {
                if (u.min.y - start.min.y).abs() < (d.min.y - start.min.y).abs() {
                    u
                } else {
                    d
                }
            }
            (Some(d), None) => d,
            (None, Some(u)) => u,
            // Nowhere to go (a panel too small to stack five readouts in):
            // in view and over its blank beats pushed out of the frame.
            (None, None) => start,
        });
    }
    placed
}

/// Push `r` along `dir` past one blocker at a time until it is clear, or `None`
/// if it leaves `safe` first. Each step moves strictly in `dir`, so a blocker
/// already passed cannot come back and the walk terminates.
fn slide_clear(mut r: Rect, blocked: &[Rect], safe: Rect, dir: f32) -> Option<Rect> {
    for _ in 0..=blocked.len() {
        let Some(hit) = blocked.iter().find(|b| b.intersects(r)) else {
            return Some(r);
        };
        let dy = if dir > 0.0 {
            hit.max.y + LABEL_GAP - r.min.y
        } else {
            hit.min.y - LABEL_GAP - r.max.y
        };
        r = r.translate(Vec2::new(0.0, dy));
        if r.min.y < safe.min.y || r.max.y > safe.max.y {
            return None;
        }
    }
    None
}

/// Shift `r` back inside `safe`, never resizing it (the text is already laid
/// out) -- a label wider than `safe` splits the difference.
fn clamp_inside(r: Rect, safe: Rect) -> Rect {
    let dx = (safe.min.x - r.min.x).max(0.0) + (safe.max.x - r.max.x).min(0.0);
    let dy = (safe.min.y - r.min.y).max(0.0) + (safe.max.y - r.max.y).min(0.0);
    r.translate(Vec2::new(dx, dy))
}

#[cfg(test)]
mod marker_label_tests {
    use super::*;

    /// A 400 px plot, the size the diagnostic board draws the hodograph at.
    fn plot() -> Rect {
        Rect::from_min_size(Pos2::new(1350.0, 45.0), Vec2::splat(400.0))
    }

    /// The five readouts of the plate that motivated this, in queue order, all
    /// anchored to `marker`: the weak-flow case where RM, LM, both Corfidi
    /// nodes and the mean wind land on the same pixel. Sizes and offsets are
    /// the laid-out widths of "240/19 RM", "58/10 LM", "UP=165/13",
    /// "DN=139/10" and "0/13" at the label font.
    fn coincident_group(marker: Pos2) -> (Vec<Rect>, Vec<Rect>) {
        let queued = [
            (29.5, 11.0, 56.0, 14.0),
            (29.5, 11.0, 53.0, 14.0),
            (31.0, 8.0, 58.0, 14.0),
            (31.0, 8.0, 58.0, 14.0),
            (21.0, 11.0, 30.0, 14.0),
        ];
        let wanted = queued
            .iter()
            .map(|(dx, dy, w, h)| {
                Rect::from_center_size(marker + Vec2::new(*dx, *dy), Vec2::new(*w, *h))
            })
            .collect();
        let markers = vec![
            Rect::from_center_size(marker, Vec2::splat(2.0 * MARKER_KEEP_OUT));
            queued.len()
        ];
        (wanted, markers)
    }

    fn pairs(placed: &[Rect]) -> impl Iterator<Item = (usize, usize)> {
        let n = placed.len();
        (0..n).flat_map(move |i| (i + 1..n).map(move |j| (i, j)))
    }

    #[test]
    fn coincident_storm_motion_labels_are_spread_until_none_overlaps() {
        let plot = plot();
        let (wanted, markers) = coincident_group(plot.center());
        assert!(
            labels_are_tangled(&wanted, &[]),
            "five labels on one marker must count as tangled"
        );

        let placed = spread_labels(&wanted, &markers, &[], plot.shrink(LABEL_GAP));

        assert_eq!(placed.len(), wanted.len());
        for (i, j) in pairs(&placed) {
            assert!(
                !placed[i].intersects(placed[j]),
                "labels {i} and {j} still overlap: {:?} vs {:?}",
                placed[i],
                placed[j]
            );
        }
        for (i, r) in placed.iter().enumerate() {
            assert_eq!(r.size(), wanted[i].size(), "label {i} was resized");
            assert_eq!(r.min.x, wanted[i].min.x, "label {i} moved off its marker");
        }
    }

    #[test]
    fn a_spread_label_stays_inside_the_frame_even_against_the_bottom_edge() {
        let plot = plot();
        // Markers this low leave no room below for the stack, so it has to go up.
        let (wanted, markers) = coincident_group(Pos2::new(plot.center().x, plot.max.y - 12.0));
        let safe = plot.shrink(LABEL_GAP);

        let placed = spread_labels(&wanted, &markers, &[], safe);

        for (i, r) in placed.iter().enumerate() {
            assert!(safe.contains_rect(*r), "label {i} left the frame: {r:?}");
        }
        for (i, j) in pairs(&placed) {
            assert!(!placed[i].intersects(placed[j]), "labels {i} and {j} overlap");
        }
    }

    #[test]
    fn a_spread_label_never_covers_someone_elses_marker() {
        let plot = plot();
        let (wanted, markers) = coincident_group(plot.center());

        let placed = spread_labels(&wanted, &markers, &[], plot.shrink(LABEL_GAP));

        for (i, r) in placed.iter().enumerate() {
            for (j, m) in markers.iter().enumerate() {
                assert!(
                    i == j || !r.intersects(*m),
                    "label {i} covers marker {j}: {r:?} vs {m:?}"
                );
            }
        }
    }

    /// The critical-angle blank: fixed to the frame at (15, hgt - 36).
    fn crit_readout(plot: Rect) -> Rect {
        Rect::from_min_size(
            plot.min + Vec2::new(15.0, plot.height() - 36.0),
            Vec2::new(100.0, 21.0),
        )
    }

    #[test]
    fn a_label_landing_on_the_critical_angle_readout_is_moved_off_it() {
        let plot = plot();
        let readout = crit_readout(plot);
        let marker = readout.center();
        let wanted = vec![Rect::from_center_size(
            marker + Vec2::new(29.5, 11.0),
            Vec2::new(56.0, 14.0),
        )];
        let markers = vec![Rect::from_center_size(marker, Vec2::splat(2.0 * MARKER_KEEP_OUT))];
        assert!(labels_are_tangled(&wanted, &[readout]));

        let placed = spread_labels(&wanted, &markers, &[readout], plot.shrink(LABEL_GAP));

        assert!(!placed[0].intersects(readout), "{:?}", placed[0]);
        assert!(plot.shrink(LABEL_GAP).contains_rect(placed[0]));
    }

    #[test]
    fn labels_on_a_strong_flow_plate_take_the_unchanged_path() {
        // Markers tens of knots apart at the default zoom, which is the case
        // that must stay pixel-for-pixel what it was: `labels_are_tangled` is
        // the only gate on that, so assert on it.
        let plot = plot();
        let wanted: Vec<Rect> = [
            Vec2::new(-60.0, -80.0),
            Vec2::new(80.0, 20.0),
            Vec2::new(-10.0, 60.0),
        ]
        .iter()
        .map(|d| Rect::from_center_size(plot.center() + *d + Vec2::new(29.5, 11.0), Vec2::new(56.0, 14.0)))
        .collect();

        assert!(!labels_are_tangled(&wanted, &[crit_readout(plot)]));
    }
}

// ----------------------------------------------------------------------
// Locator inset (port of sharpmod.viz.hodo_locator.draw_hodo_locator)
// ----------------------------------------------------------------------

/// Geographic extent of the inset. The Python original shows a ~1.4 deg-tall
/// county view fetched from TIGERweb; with the embedded state-boundary
/// basemap the window is widened so state lines are actually in view.
fn zoom_bounds(lat: f64, lon: f64) -> (f64, f64, f64, f64) {
    let half_lat = 4.0;
    let cos_lat = lat.to_radians().cos().max(0.35);
    let half_lon = half_lat * 1.35 / cos_lat;
    (lon - half_lon, lat - half_lat, lon + half_lon, lat + half_lat)
}

fn draw_locator(p: &Painter, g: &Geom, prof: &Profile, style: &SkewTStyle) {
    let lat = prof.inner.station.latitude;
    let lon = prof.inner.station.longitude;
    if !lat.is_finite() || !lon.is_finite() || lat.abs() > 90.0 || lon.abs() > 180.0 {
        return;
    }
    // Share the hodograph's upper-left corner (frame border + 1 px).
    let (frame_left, frame_top) = (1.0f64, 1.0f64);
    let avail_w = (g.wid - frame_left - 8.0).max(0.0);
    let avail_h = (g.hgt - frame_top - 8.0).max(0.0);
    let width = (g.wid * 0.29).floor().max(150.0).min(250.0).min(avail_w);
    let height = (width * 0.64).floor().max(96.0).min(avail_h);
    if width < 110.0 || height < 72.0 {
        return;
    }
    let rect = Rect::from_min_size(
        g.pt(frame_left, frame_top),
        Vec2::new(width as f32, height as f32),
    );
    p.rect_filled(rect, 0.0, MAP_FILL);
    p.rect_stroke(rect, 0.0, Stroke::new(1.25, style.fg_color), StrokeKind::Middle);

    let interior = rect.shrink(5.0);
    let mp = p.with_clip_rect(interior);
    let (west, south, east, north) = zoom_bounds(lat, lon);
    let map_point = |plon: f64, plat: f64| -> Pos2 {
        Pos2::new(
            interior.min.x + ((plon - west) / (east - west) * interior.width() as f64) as f32,
            interior.min.y + ((north - plat) / (north - south) * interior.height() as f64) as f32,
        )
    };

    // State boundary / coastline polylines (embedded basemap).
    let outline = Stroke::new(1.0, style.fg_color);
    for seg in hodo_map_data::SEGMENTS {
        // Skip segments entirely outside the view (cheap bbox reject).
        let visible = seg.iter().any(|(slon, slat)| {
            (*slon as f64) >= west - 6.0
                && (*slon as f64) <= east + 6.0
                && (*slat as f64) >= south - 6.0
                && (*slat as f64) <= north + 6.0
        });
        if !visible {
            continue;
        }
        let pts: Vec<Pos2> = seg
            .iter()
            .map(|(slon, slat)| map_point(*slon as f64, *slat as f64))
            .collect();
        if pts.len() >= 2 {
            mp.add(Shape::line(pts, outline));
        }
    }

    // Crosshair at the sounding point.
    let c = map_point(lon, lat);
    let marker = Stroke::new(1.4, MAP_POINT_COLOR);
    mp.circle(c, 4.0, MAP_FILL, marker);
    mp.line_segment([c - Vec2::new(7.0, 0.0), c + Vec2::new(7.0, 0.0)], marker);
    mp.line_segment([c - Vec2::new(0.0, 7.0), c + Vec2::new(0.0, 7.0)], marker);
}

/// Linked cursor marker: the wind at `h_agl` (m) highlighted on the hodograph
/// with a wind/height readout, driven by hovering the skew-T (see
/// `SoundingView`).
pub(crate) fn cursor_marker(
    painter: &Painter,
    rect: Rect,
    prof: &Profile,
    style: &SkewTStyle,
    h_agl: f64,
    zoom_kts: f64,
) {
    let g = Geom::with_zoom(rect, zoom_kts);
    let inner = &prof.inner;
    let pres = inner.pres_at_height(inner.to_msl(h_agl));
    if !pres.is_finite() {
        return;
    }
    let (u, v) = inner.interp_wind(pres.min(inner.pres[inner.sfc]));
    if !u.is_finite() || !v.is_finite() {
        return;
    }
    let p = painter.with_clip_rect(rect);
    let (x, y) = g.uv_to_pix(u, v);
    let c = g.pt(x, y);
    p.circle_stroke(c, 5.0, Stroke::new(1.6, style.fg_color));
    p.circle_stroke(c, 1.0, Stroke::new(2.0, style.fg_color));

    let (wdir, wspd) = inner.interp_vec(pres.min(inner.pres[inner.sfc]));
    let text = format!(
        "{:.1} km  {}/{}",
        h_agl / 1000.0,
        crate::utils::int2str(wdir),
        crate::utils::int2str(wspd)
    );
    let font = style.regular_font(11.0);
    let galley = p.layout_no_wrap(text, font, style.fg_color);
    let mut anchor = c + Vec2::new(8.0, -8.0 - galley.size().y);
    if anchor.x + galley.size().x > rect.max.x - 2.0 {
        anchor.x = c.x - 8.0 - galley.size().x;
    }
    if anchor.y < rect.min.y + 2.0 {
        anchor.y = c.y + 8.0;
    }
    let bg = Rect::from_min_size(anchor, galley.size() + Vec2::new(4.0, 2.0));
    p.rect_filled(bg, 0.0, style.bg_color);
    p.galley(bg.min + Vec2::new(2.0, 1.0), galley, style.fg_color);
}
