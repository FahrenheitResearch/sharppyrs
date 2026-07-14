//! PANEL PORT PENDING (see PORTING.md).

use egui::{Painter, Rect};

use crate::derived::DerivedParams;
use crate::skewt::SkewTStyle;
use crate::Profile;

/// Draw this panel into `rect`.
#[allow(unused_variables)]
pub fn draw(painter: &Painter, rect: Rect, prof: &Profile, dv: &DerivedParams, style: &SkewTStyle) {
    // Pending port: draw the panel frame so composition can be previewed.
    painter.rect_stroke(
        rect.shrink(0.5),
        0.0,
        egui::Stroke::new(1.0, style.fg_color),
        egui::StrokeKind::Inside,
    );
}
