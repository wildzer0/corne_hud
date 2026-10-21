//! Keyboard rendering: KLE geometry (with rotations) and legends.

use super::theme::{mix, Palette};
use crate::hid::protocol::HudState;
use crate::kle::{KeyRole, KleKey};
use crate::model::Keymap;
use crate::vial::keycode::KC_TRNS;
use crate::vial::legend::{legend, LegendContext, LegendKind, Text};
use egui::epaint::TextShape;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};

/// Gap between keys, in key units.
const GAP: f32 = 0.06;

pub struct KeyboardView<'a> {
    pub keys: &'a [KleKey],
    pub keymap: &'a Keymap,
    /// Layer whose legends are shown.
    pub layer: u8,
    /// Active layers used to resolve `KC_TRNS`.
    pub active_mask: u32,
    pub state: Option<&'a HudState>,
    pub legends: &'a LegendContext<'a>,
    pub layer_color: &'a dyn Fn(u8) -> Option<Color32>,
}

struct Transform {
    origin: Pos2,
    min: (f32, f32),
    scale: f32,
}

impl Transform {
    fn fit(keys: &[KleKey], rect: Rect) -> Option<Self> {
        let mut min = (f32::MAX, f32::MAX);
        let mut max = (f32::MIN, f32::MIN);
        for k in keys {
            for (x, y) in k.corners() {
                min = (min.0.min(x), min.1.min(y));
                max = (max.0.max(x), max.1.max(y));
            }
        }
        let (w, h) = (max.0 - min.0, max.1 - min.1);
        if !(w > 0.0 && h > 0.0) {
            return None;
        }
        let scale = (rect.width() / w).min(rect.height() / h);
        let origin = rect.center() - Vec2::new(w * scale / 2.0, h * scale / 2.0);
        Some(Transform { origin, min, scale })
    }

    fn pos(&self, (x, y): (f32, f32)) -> Pos2 {
        self.origin + Vec2::new((x - self.min.0) * self.scale, (y - self.min.1) * self.scale)
    }
}

fn inset(k: &KleKey) -> KleKey {
    let g = GAP.min(k.w / 4.0).min(k.h / 4.0);
    KleKey { x: k.x + g, y: k.y + g, w: k.w - 2.0 * g, h: k.h - 2.0 * g, ..k.clone() }
}

fn contains(poly: &[Pos2], p: Pos2) -> bool {
    let n = poly.len();
    let mut sign = 0.0f32;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let cross = (b - a).x * (p - a).y - (b - a).y * (p - a).x;
        if cross != 0.0 {
            if sign != 0.0 && cross.signum() != sign {
                return false;
            }
            sign = cross.signum();
        }
    }
    true
}

/// Draws `text` centered at `center`, rotated by `angle`, shrunk to fit `max_width`.
fn draw_text(painter: &Painter, text: &Text, center: Pos2, angle: f32, size: f32, max_width: f32, color: Color32) {
    let font = FontId::proportional(size);
    let s = if painter.fonts_mut(|f| f.has_glyphs(&font, &text.symbol)) { &text.symbol } else { &text.text };
    if s.is_empty() {
        return;
    }
    let mut galley = painter.layout_no_wrap(s.clone(), font, color);
    if galley.size().x > max_width && galley.size().x > 0.0 {
        let fitted = (size * max_width / galley.size().x).max(size * 0.45);
        galley = painter.layout_no_wrap(s.clone(), FontId::proportional(fitted), color);
    }
    let pos = center - galley.size() / 2.0;
    painter.add(TextShape::new(pos, galley, color).with_angle_and_anchor(angle, Align2::CENTER_CENTER));
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Draws the keyboard in `rect`. Returns the tooltip of the hovered key.
pub fn draw(
    painter: &Painter,
    rect: Rect,
    view: &KeyboardView,
    palette: &Palette,
    hover: Option<Pos2>,
) -> Option<String> {
    let tf = Transform::fit(view.keys, rect)?;
    let mut tooltip = None;

    for key in view.keys {
        let role = key.role();
        let poly: Vec<Pos2> = inset(key).corners().iter().map(|&c| tf.pos(c)).collect();
        let center = tf.pos(key.center());
        let angle = key.r.to_radians();
        let (w, h) = ((key.w - 2.0 * GAP) * tf.scale, (key.h - 2.0 * GAP) * tf.scale);

        let Some(KeyRole::Matrix { row, col }) = role else {
            // encoders: a simple knob
            painter.circle(center, w.min(h) / 2.0, palette.key_empty, Stroke::new(1.0, palette.key_border));
            continue;
        };

        let own = view.keymap.get(view.layer, row, col);
        let (code, _from) = view.keymap.resolve(view.layer, view.active_mask, row, col);
        let leg = legend(code, view.legends);
        let transparent = own == KC_TRNS;
        let pressed = view.state.is_some_and(|s| s.is_pressed(row, col));

        let mut fill = match leg.kind {
            LegendKind::Empty => palette.key_empty,
            LegendKind::Layer(l) => (view.layer_color)(l).map(|c| mix(palette.key, c, 0.5)).unwrap_or(palette.key),
            _ => palette.key,
        };
        if transparent {
            fill = mix(fill, palette.key_empty, 0.5);
        }
        if pressed {
            fill = mix(fill, palette.pressed, 0.65);
        }
        let text_color = match (pressed, transparent) {
            (true, _) => Color32::from_rgb(20, 20, 24),
            (false, true) => palette.text_dim,
            (false, false) => palette.text,
        };
        painter.add(Shape::convex_polygon(poly.clone(), fill, Stroke::new(1.0, palette.key_border)));

        let base = tf.scale * 0.30;
        match &leg.hold {
            Some(hold) => {
                draw_text(
                    painter,
                    &leg.tap,
                    center + rotate(Vec2::new(0.0, -h * 0.12), angle),
                    angle,
                    base,
                    w * 0.88,
                    text_color,
                );
                draw_text(
                    painter,
                    hold,
                    center + rotate(Vec2::new(0.0, h * 0.28), angle),
                    angle,
                    base * 0.55,
                    w * 0.9,
                    text_color.gamma_multiply(0.75),
                );
            }
            None => draw_text(painter, &leg.tap, center, angle, base, w * 0.88, text_color),
        }

        if hover.is_some_and(|p| contains(&poly, p)) {
            let source = if transparent { " (trasparente)" } else { "" };
            tooltip = Some(format!("{}{source}\nriga {row}, colonna {col}", leg.tooltip));
        }
    }
    tooltip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_in_convex_polygon() {
        let sq = [Pos2::new(0.0, 0.0), Pos2::new(2.0, 0.0), Pos2::new(2.0, 2.0), Pos2::new(0.0, 2.0)];
        assert!(contains(&sq, Pos2::new(1.0, 1.0)));
        assert!(!contains(&sq, Pos2::new(3.0, 1.0)));
        assert!(!contains(&sq, Pos2::new(-0.1, 1.0)));
    }
}
