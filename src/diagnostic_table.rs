//! Host-supplied scalar diagnostic tables for the SPC sounding window.
//!
//! The native SHARPpy panels remain the default. A host can opt into these
//! tables panel-by-panel after it has resolved its own stable diagnostic IDs
//! into display-ready rows. Keeping evaluation outside this crate lets model
//! applications add private or user-defined diagnostics without teaching the
//! sounding renderer about their formula engines.

use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::SkewTStyle;

const RULE: Color32 = Color32::from_rgb(0x8A, 0x8A, 0x8A);
const SECTION: Color32 = Color32::from_rgb(0x04, 0xDB, 0xD8);

/// One of the three native scalar-table panels that a host may replace.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DiagnosticTablePanelKind {
    Convective,
    Kinematics,
    Severe,
}

/// A fully resolved, display-ready scalar readout.
///
/// `value` should contain only the value (or `"--"` when unavailable); the
/// renderer gives `unit` a smaller type size. `color = None` uses the current
/// sounding foreground color.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticTableRow {
    pub label: String,
    pub value: String,
    pub unit: String,
    pub color: Option<Color32>,
}

impl DiagnosticTableRow {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            unit: String::new(),
            color: None,
        }
    }

    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

/// An independently titled list of rows inside a panel.
///
/// Rows retain their order and flow top-to-bottom, then left-to-right through
/// `columns`. Hosts normally use one to three columns. Values outside 1..=4
/// are safely clamped by the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticTableSection {
    pub title: String,
    pub rows: Vec<DiagnosticTableRow>,
    pub columns: usize,
}

impl DiagnosticTableSection {
    pub fn new(title: impl Into<String>, rows: Vec<DiagnosticTableRow>) -> Self {
        Self {
            title: title.into(),
            rows,
            columns: 1,
        }
    }

    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.clamp(1, 4);
        self
    }
}

/// Host override for one native scalar-table panel. An empty `sections` list
/// intentionally renders an empty panel, so a user can remove every readout.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticTablePanel {
    pub kind: DiagnosticTablePanelKind,
    pub title: String,
    pub sections: Vec<DiagnosticTableSection>,
}

impl DiagnosticTablePanel {
    pub fn new(
        kind: DiagnosticTablePanelKind,
        title: impl Into<String>,
        sections: Vec<DiagnosticTableSection>,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            sections,
        }
    }
}

/// Per-panel host overrides. Omitted panels continue to use their exact native
/// SHARPpy renderer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiagnosticTableBoard {
    pub panels: Vec<DiagnosticTablePanel>,
}

impl DiagnosticTableBoard {
    pub fn panel(&self, kind: DiagnosticTablePanelKind) -> Option<&DiagnosticTablePanel> {
        self.panels.iter().find(|panel| panel.kind == kind)
    }
}

fn rows_per_column(section: &DiagnosticTableSection) -> usize {
    let columns = section.columns.clamp(1, 4);
    section.rows.len().div_ceil(columns).max(1)
}

fn text_width(painter: &Painter, font: &FontId, text: &str) -> f32 {
    painter
        .layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE)
        .size()
        .x
}

fn fitted_font(painter: &Painter, mut font: FontId, text: &str, width: f32) -> FontId {
    while font.size > 1.0 && text_width(painter, &font, text) > width {
        font.size -= 0.5;
    }
    font
}

fn draw_row(
    painter: &Painter,
    rect: Rect,
    row: &DiagnosticTableRow,
    font: &FontId,
    style: &SkewTStyle,
) {
    if rect.width() <= 2.0 || rect.height() <= 2.0 {
        return;
    }
    let color = row.color.unwrap_or(style.fg_color);
    let prefix = if row.label.is_empty() {
        String::new()
    } else {
        format!("{} = ", row.label)
    };
    let unit = if row.value == "--" || row.unit.is_empty() {
        String::new()
    } else {
        format!(" {}", row.unit)
    };
    let full = format!("{prefix}{}{unit}", row.value);
    let fitted = fitted_font(painter, font.clone(), &full, rect.width());
    let unit_font = FontId::new((fitted.size * 0.72).max(1.0), fitted.family.clone());
    let prefix_width = text_width(painter, &fitted, &prefix);
    let value_width = text_width(painter, &fitted, &row.value);
    let unit_width = text_width(painter, &unit_font, &unit);
    let total = prefix_width + value_width + unit_width;
    if total > rect.width() + 0.5 {
        painter.text(
            pos2(rect.left(), rect.center().y),
            Align2::LEFT_CENTER,
            full,
            fitted,
            color,
        );
        return;
    }
    let y = rect.center().y;
    painter.text(
        pos2(rect.left(), y),
        Align2::LEFT_CENTER,
        prefix,
        fitted.clone(),
        color,
    );
    painter.text(
        pos2(rect.left() + prefix_width, y),
        Align2::LEFT_CENTER,
        &row.value,
        fitted,
        color,
    );
    if !unit.is_empty() {
        painter.text(
            pos2(rect.left() + prefix_width + value_width, y),
            Align2::LEFT_CENTER,
            unit,
            unit_font,
            color,
        );
    }
}

/// Draw one configured panel. Kept crate-visible because the public contract
/// is the structured data above; `SoundingView` owns placement and clipping.
pub(crate) fn draw(
    painter: &Painter,
    rect: Rect,
    panel: &DiagnosticTablePanel,
    style: &SkewTStyle,
) {
    if rect.width() <= 6.0 || rect.height() <= 6.0 {
        return;
    }
    let painter = painter.with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, style.bg_color);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, RULE), StrokeKind::Inside);
    let content = rect.shrink2(vec2(5.0, 3.0));
    let sections: Vec<&DiagnosticTableSection> = panel
        .sections
        .iter()
        .filter(|section| !section.rows.is_empty())
        .collect();
    let panel_title_lines = usize::from(!panel.title.trim().is_empty());
    let section_lines: usize = sections
        .iter()
        .map(|section| rows_per_column(section) + usize::from(!section.title.trim().is_empty()))
        .sum();
    let separator_units = sections.len().saturating_sub(1) as f32 * 0.35;
    let line_units = (panel_title_lines + section_lines) as f32 + separator_units;
    if line_units <= 0.0 {
        return;
    }
    let row_h = (content.height() / line_units).max(1.0);
    let regular = style.regular_font((row_h * 0.74).clamp(1.0, 18.0));
    let heading = style.bold_font((row_h * 0.72).clamp(1.0, 18.0));
    let mut y = content.top();

    if panel_title_lines > 0 {
        painter.text(
            pos2(content.left(), y + row_h * 0.5),
            Align2::LEFT_CENTER,
            panel.title.trim(),
            heading.clone(),
            style.fg_color,
        );
        y += row_h;
    }

    for (section_index, section) in sections.iter().enumerate() {
        if section_index > 0 {
            y += row_h * 0.15;
            painter.line_segment(
                [pos2(content.left(), y), pos2(content.right(), y)],
                Stroke::new(1.0, RULE),
            );
            y += row_h * 0.20;
        }
        if !section.title.trim().is_empty() {
            painter.text(
                pos2(content.left(), y + row_h * 0.5),
                Align2::LEFT_CENTER,
                section.title.trim(),
                heading.clone(),
                SECTION,
            );
            y += row_h;
        }
        let columns = section.columns.clamp(1, 4);
        let rows_per_column = rows_per_column(section);
        let column_width = content.width() / columns as f32;
        for (index, row) in section.rows.iter().enumerate() {
            let column = index / rows_per_column;
            let row_index = index % rows_per_column;
            let cell = Rect::from_min_size(
                pos2(
                    content.left() + column as f32 * column_width + 2.0,
                    y + row_index as f32 * row_h,
                ),
                vec2((column_width - 5.0).max(1.0), row_h),
            );
            draw_row(&painter, cell, row, &regular, style);
        }
        y += rows_per_column as f32 * row_h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_lookup_is_panel_specific_and_omissions_remain_native() {
        let board = DiagnosticTableBoard {
            panels: vec![DiagnosticTablePanel::new(
                DiagnosticTablePanelKind::Kinematics,
                "Wind",
                Vec::new(),
            )],
        };
        assert!(board.panel(DiagnosticTablePanelKind::Convective).is_none());
        assert_eq!(
            board
                .panel(DiagnosticTablePanelKind::Kinematics)
                .map(|panel| panel.title.as_str()),
            Some("Wind")
        );
    }

    #[test]
    fn rows_flow_top_to_bottom_through_bounded_columns() {
        let rows = (0..7)
            .map(|index| DiagnosticTableRow::new(format!("R{index}"), index.to_string()))
            .collect();
        let section = DiagnosticTableSection::new("", rows).columns(3);
        assert_eq!(section.columns, 3);
        assert_eq!(rows_per_column(&section), 3);
        assert_eq!(
            DiagnosticTableSection::new("", Vec::new())
                .columns(99)
                .columns,
            4
        );
    }

    #[test]
    fn missing_values_do_not_repeat_units() {
        let row = DiagnosticTableRow::new("CAPE", "--").unit("J/kg");
        assert_eq!(row.value, "--");
        assert_eq!(row.unit, "J/kg");
    }
}
