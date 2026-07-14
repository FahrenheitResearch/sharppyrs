//! The full SPC sounding window: skew-T + speed/advection strips, hodograph
//! with locator inset, storm slinky / theta-e / SR-winds / hazard row, and
//! the bottom index-board band — laid out like the vendored `SPCWidget` grid
//! with the SHARPpy-Reimagined modifications (see PORTING.md).

use egui::{Align2, Rect, Response, Sense, Ui, Vec2, Widget};

use crate::derived::DerivedParams;
use crate::panels;
use crate::profile::{ParcelType, Profile};
use crate::skewt::{SkewT, SkewTStyle};

/// The complete sounding window. Compute [`DerivedParams`] once (it is not
/// cheap) and keep it alongside the profile.
pub struct SoundingView<'a> {
    prof: &'a Profile,
    derived: &'a DerivedParams,
    title: String,
    brand: Option<String>,
    parcel: ParcelType,
    style: SkewTStyle,
    size: Option<Vec2>,
}

impl<'a> SoundingView<'a> {
    pub fn new(prof: &'a Profile, derived: &'a DerivedParams) -> Self {
        SoundingView {
            prof,
            derived,
            title: String::new(),
            brand: None,
            parcel: ParcelType::MostUnstable,
            style: SkewTStyle::default(),
            size: None,
        }
    }

    /// Skew-T title (top-left).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Brand text drawn top-right above the hodograph column.
    pub fn brand(mut self, brand: impl Into<String>) -> Self {
        self.brand = Some(brand.into());
        self
    }

    pub fn parcel(mut self, parcel: ParcelType) -> Self {
        self.parcel = parcel;
        self
    }

    pub fn style(mut self, style: SkewTStyle) -> Self {
        self.style = style;
        self
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = Some(size);
        self
    }
}

impl Widget for SoundingView<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = self.size.unwrap_or_else(|| ui.available_size());
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        if !ui.is_rect_visible(rect) || rect.width() < 200.0 || rect.height() < 150.0 {
            return response;
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, self.style.bg_color);

        let w = rect.width();
        let h = rect.height();

        // Vertical split: top (skew-T + upper-right) / bottom index band,
        // matching the reference proportions (~67% / 33%).
        let band_top = rect.min.y + h * 0.67;
        // Horizontal split of the top: skew-T column ~46%.
        let skew_right = rect.min.x + w * 0.46;

        // --- Skew-T (its own Widget; place it in its cell). ---
        let skew_rect = Rect::from_min_max(rect.min, egui::pos2(skew_right, band_top));
        let mut skew_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(skew_rect)
                .layout(egui::Layout::default()),
        );
        skew_ui.add(
            SkewT::new(self.prof)
                .parcel(self.parcel)
                .title(self.title.clone())
                .style(self.style.clone())
                .size(skew_rect.size()),
        );

        // --- Upper right: brand band + grid2. ---
        let ur = Rect::from_min_max(egui::pos2(skew_right, rect.min.y), egui::pos2(rect.max.x, band_top));
        let brand_h = 16.0f32;
        if let Some(brand) = &self.brand {
            painter.text(
                egui::pos2(ur.max.x - 4.0, ur.min.y + 2.0),
                Align2::RIGHT_TOP,
                brand,
                egui::FontId::new(11.0, self.style.font_regular.clone()),
                self.style.fg_color,
            );
        }
        let g2 = Rect::from_min_max(egui::pos2(ur.min.x, ur.min.y + brand_h), ur.max);
        // grid2: 29 columns x 11 rows.
        let colw = g2.width() / 29.0;
        let rowh = g2.height() / 11.0;
        let cell = |c0: f32, r0: f32, cs: f32, rs: f32| {
            Rect::from_min_size(
                egui::pos2(g2.min.x + c0 * colw, g2.min.y + r0 * rowh),
                Vec2::new(cs * colw, rs * rowh),
            )
        };
        let speed_rect = cell(0.0, 0.0, 3.0, 11.0);
        let adv_rect = cell(3.0, 0.0, 2.0, 11.0);
        let hodo_rect = cell(5.0, 0.0, 24.0, 8.0);
        let slinky_rect = cell(5.0, 8.0, 6.0, 3.0);
        let thetae_rect = cell(11.0, 8.0, 6.0, 3.0);
        let srwinds_rect = cell(17.0, 8.0, 6.0, 3.0);
        let hazard_rect = cell(23.0, 8.0, 6.0, 3.0);

        let dv = self.derived;
        let st = &self.style;
        panels::speed::draw(&painter, speed_rect, self.prof, dv, st);
        panels::advection::draw(&painter, adv_rect, self.prof, dv, st);
        panels::hodo::draw(&painter, hodo_rect, self.prof, dv, st);
        panels::slinky::draw(&painter, slinky_rect, self.prof, dv, st);
        panels::thetae::draw(&painter, thetae_rect, self.prof, dv, st);
        panels::srwinds::draw(&painter, srwinds_rect, self.prof, dv, st);
        panels::hazard::draw(&painter, hazard_rect, self.prof, dv, st);

        // --- Bottom band: index board / streamwiseness / STP (61/14/25%). ---
        let band = Rect::from_min_max(egui::pos2(rect.min.x, band_top), rect.max);
        let x1 = band.min.x + band.width() * 0.61;
        let x2 = band.min.x + band.width() * 0.75;
        let board_rect = Rect::from_min_max(band.min, egui::pos2(x1, band.max.y));
        let stream_rect = Rect::from_min_max(egui::pos2(x1, band.min.y), egui::pos2(x2, band.max.y));
        let stp_rect = Rect::from_min_max(egui::pos2(x2, band.min.y), band.max);
        panels::index_board::draw(&painter, board_rect, self.prof, dv, st);
        panels::streamwiseness::draw(&painter, stream_rect, self.prof, dv, st);
        panels::stp::draw(&painter, stp_rect, self.prof, dv, st);

        response
    }
}
