//! Forecaster's notes panel: the host's prose about WHY this sounding is worth
//! posting, wrapped into whatever cell the layout gives it.
//!
//! Not a port of anything in SHARPpy — the original window has no such cell.
//! The note used to be fitted onto the single upper-right header line and
//! elided there, which holds about one clause.
//!
//! Wrapping is egui's own [`LayoutJob`] machinery rather than a character
//! count: the sounding faces are proportional, so "90 characters" is a
//! different width in every note. Text that still does not fit is elided with
//! a trailing `…`, because the cell is clipped — an overflowing row would be
//! prose lost without a trace.

use std::sync::Arc;

use egui::text::{Galley, LayoutJob};
use egui::{Align2, Color32, Painter, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::skewt::SkewTStyle;

/// The panel's own label, like every other panel names itself.
const TITLE: &str = "Notes";

/// Point size of the note, title and body alike. The note used to be drawn on
/// the upper-right header line at 11 pt, so keeping that size reads as the same
/// text moved rather than restyled.
const NOTE_PT: f32 = 11.0;

/// Inner margin, matching the host-supplied diagnostic tables.
const PAD: Vec2 = vec2(5.0, 3.0);

/// Frame and separator grey, shared with the other bordered text panels.
const RULE: Color32 = Color32::from_rgb(0x8A, 0x8A, 0x8A);

/// Draw the host's note into `rect`.
///
/// An empty note still draws the cell: frame, title and rule, with no body. The
/// panel used to return early and leave bare background, which on a plate where
/// every other cell is framed reads as a rendering fault rather than as an empty
/// cell — and empty is the COMMON case, since a note is opt-in. What the early
/// return was right about is that no placeholder TEXT belongs here; drawing the
/// host's silence as content would be worse than drawing nothing.
pub fn draw(painter: &Painter, rect: Rect, style: &SkewTStyle, note: &str) {
    let p = painter.with_clip_rect(rect);
    p.rect_filled(rect, 0.0, style.bg_color);

    let note = note.trim();
    if rect.width() <= 6.0 || rect.height() <= 6.0 {
        return;
    }
    p.rect_stroke(rect, 0.0, Stroke::new(1.0, RULE), StrokeKind::Inside);

    let (rule_y, body) = rule_and_body(&p, rect, style);
    p.text(
        rect.shrink2(PAD).left_top(),
        Align2::LEFT_TOP,
        TITLE,
        style.bold_font(NOTE_PT),
        style.fg_color,
    );
    p.line_segment(
        [pos2(body.left(), rule_y), pos2(body.right(), rule_y)],
        Stroke::new(1.0, RULE),
    );
    if note.is_empty() {
        return;
    }
    if let Some(galley) = fitted_note(&p, body, style, note) {
        p.galley(body.min, galley, style.fg_color);
    }
}

/// The separator rule's height and the area the note's rows get under it. One
/// function so a test measures the very width the panel wraps to.
fn rule_and_body(painter: &Painter, rect: Rect, style: &SkewTStyle) -> (f32, Rect) {
    let content = rect.shrink2(PAD);
    let title_h = painter.fonts_mut(|f| f.row_height(&style.bold_font(NOTE_PT)));
    // Tight clearances around the rule: the cell holds eight or nine rows, so
    // anything roomier is paid for out of the note.
    let rule_y = content.top() + title_h + 2.0;
    (
        rule_y,
        Rect::from_min_max(pos2(content.left(), rule_y + 3.0), content.max),
    )
}

/// Wrap `note` into `body`, eliding at the last row that fits. `None` when not
/// even one row does.
fn fitted_note(
    painter: &Painter,
    body: Rect,
    style: &SkewTStyle,
    note: &str,
) -> Option<Arc<Galley>> {
    let font = style.regular_font(NOTE_PT);
    let row_h = painter.fonts_mut(|f| f.row_height(&font));
    if body.width() < 1.0 || row_h <= 0.0 || body.height() < row_h {
        return None;
    }
    let mut rows = (body.height() / row_h) as usize;
    loop {
        let mut job =
            LayoutJob::simple(note.to_owned(), font.clone(), style.fg_color, body.width());
        job.wrap.max_rows = rows;
        // The cut has to be visible: this panel exists because a header line
        // that elided silently read as the whole note.
        job.wrap.overflow_character = Some('…');
        let galley = painter.layout_job(job);
        // A row carrying fallback glyphs can stand taller than the font's own
        // row height, so believe the laid-out size over the estimate.
        if rows <= 1 || galley.size().y <= body.height() {
            return Some(galley);
        }
        rows -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skewt::SoundingFontPreset;

    /// The slot the host has for this panel, in points.
    const HOST_SLOT: Vec2 = vec2(363.0, 146.0);

    /// A painter over a live font set: text metrics need one, and egui only
    /// builds the fonts once a pass has run.
    fn painter(ctx: &egui::Context) -> Painter {
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        Painter::new(ctx.clone(), egui::LayerId::debug(), Rect::EVERYTHING)
    }

    /// Monospace, so a row's capacity is a character count. The wrapping under
    /// test is proportional-font machinery; a test of WHERE it breaks needs
    /// widths it can predict.
    fn monospace() -> SkewTStyle {
        SkewTStyle::default().with_font_preset(SoundingFontPreset::TechnicalMonospace)
    }

    fn shape_count(note: &str) -> usize {
        let ctx = egui::Context::default();
        let p = painter(&ctx);
        draw(&p, Rect::from_min_size(pos2(0.0, 0.0), HOST_SLOT), &monospace(), note);
        let mut shapes = 0;
        p.for_each_shape(|_| shapes += 1);
        shapes
    }

    #[test]
    fn a_note_wraps_into_as_many_rows_as_its_width_allows() {
        let ctx = egui::Context::default();
        let p = painter(&ctx);
        let style = monospace();
        let char_w = p.fonts_mut(|f| f.glyph_width(&style.regular_font(NOTE_PT), 'a'));

        // Rows of four and of three four-letter words: wide enough for the last
        // word and its space, too narrow for the next one.
        let note = "aaaa ".repeat(40);
        for (words, rows) in [(4.0, 10), (3.0, 14)] {
            let width = char_w * (words * 5.0 + 0.4) + PAD.x * 2.0;
            let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 400.0));
            let (_, body) = rule_and_body(&p, rect, &style);
            let galley = fitted_note(&p, body, &style, note.trim()).expect("the rows fit");

            assert_eq!(galley.rows.len(), rows, "40 words, {words} to a row");
            assert!(!galley.elided, "a 400 pt tall cell holds {rows} rows");
        }
    }

    #[test]
    fn an_over_long_note_is_elided_instead_of_overflowing_the_cell() {
        let ctx = egui::Context::default();
        let p = painter(&ctx);
        let style = monospace();
        let rect = Rect::from_min_size(pos2(0.0, 0.0), HOST_SLOT);
        let (_, body) = rule_and_body(&p, rect, &style);
        // Prose, and one unbroken token — a pasted URL is the realistic case,
        // and it is the one a word-boundary wrapper leaves hanging out.
        for note in ["aaaa ".repeat(400), "x".repeat(4000)] {
            let galley = fitted_note(&p, body, &style, note.trim()).expect("some rows fit");

            assert!(galley.elided, "{} chars cannot fit {body:?}", note.len());
            assert!(
                galley.size().y <= body.height() && galley.rect.width() <= body.width(),
                "{:?} leaves {body:?}",
                galley.size()
            );
            assert_eq!(
                galley.rows.last().and_then(|row| row.row.glyphs.last()).map(|g| g.chr),
                Some('…'),
                "the cut has to be marked"
            );
        }
    }

    #[test]
    fn a_cell_too_short_for_one_row_draws_no_note_rather_than_a_clipped_one() {
        let ctx = egui::Context::default();
        let p = painter(&ctx);
        let style = monospace();
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(HOST_SLOT.x, 24.0));
        let (_, body) = rule_and_body(&p, rect, &style);

        assert!(body.height() < 12.0, "{body:?}");
        assert!(fitted_note(&p, body, &style, "a note").is_none());
    }

    /// Empty is the COMMON case -- a note is opt-in -- and on a plate where every
    /// other cell is framed, bare background reads as a rendering fault rather
    /// than as an empty cell. So the chrome always draws; only the body is
    /// conditional, because a placeholder would present the host's silence as
    /// content.
    #[test]
    fn an_empty_note_still_draws_the_cell_but_no_body() {
        let empty = shape_count("   \n  ");
        assert!(
            empty > 1,
            "an empty cell still gets its frame, title and rule, not just a fill"
        );
        assert!(
            shape_count("A note.") > empty,
            "and a real note adds its rows on top of that chrome"
        );
    }
}
