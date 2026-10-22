//! Keyboard rendering: KLE geometry (with rotations) and legends.

use super::theme::{mix, text_on, Palette};
use crate::kle::{KeyRole, KleKey};
use crate::labels::Labels;
use crate::model::Keymap;
use crate::vial::dynamic::Combo;
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
    /// Highlight of each matrix position, 0 (off) to 1 (pressed), for the press animation.
    pub highlight: &'a dyn Fn(u8, u8) -> f32,
    pub legends: &'a LegendContext<'a>,
    pub labels: &'a Labels,
    pub layer_color: &'a dyn Fn(u8) -> Option<Color32>,
    /// Search marks: 0 (none) to 1 (selected result).
    pub mark: &'a dyn Fn(u8, u8) -> f32,
    /// Heatmap: `(intensity 0..1, presses)` per position, when shown.
    pub heat: Option<&'a dyn Fn(u8, u8) -> (f32, u64)>,
    /// Key override firing with the modifiers held now: replacement of a keycode
    /// and a description for the tooltip.
    pub key_override: &'a dyn Fn(u16) -> Option<(u16, String)>,
    /// Combos to draw over the keys, when shown.
    pub combos: Option<&'a [Combo]>,
}

/// Cold → hot: dark blue, cyan, yellow, red.
fn heat_color(t: f32) -> Color32 {
    let stops = [
        Color32::from_rgb(30, 50, 110),
        Color32::from_rgb(0, 170, 200),
        Color32::from_rgb(240, 210, 40),
        Color32::from_rgb(230, 50, 40),
    ];
    let t = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let i = (t.floor() as usize).min(stops.len() - 2);
    mix(stops[i], stops[i + 1], t - i as f32)
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
    // lone symbols (⌫ ⏎ ␣ ...) read better a little larger than text
    let size = if s.chars().count() == 1 && !s.chars().all(char::is_alphanumeric) { size * 1.3 } else { size };
    let mut galley = painter.layout_no_wrap(s.clone(), FontId::proportional(size), color);
    if galley.size().x > max_width && galley.size().x > 0.0 {
        let fitted = (size * max_width / galley.size().x).max(size * 0.45);
        galley = painter.layout_no_wrap(s.clone(), FontId::proportional(fitted), color);
    }
    let pos = center - galley.size() / 2.0;
    painter.add(TextShape::new(pos, galley, color).with_angle_and_anchor(angle, Align2::CENTER_CENTER));
}

/// Points per rounded corner.
const CORNER_STEPS: usize = 6;

/// `poly` (convex) with its corners rounded by `radius` pixels, at most half
/// of each side. Each corner is a quadratic curve with the corner as control
/// point: the result stays convex and inside `poly`, rotated keys included.
fn rounded_polygon(poly: &[Pos2], radius: f32) -> Vec<Pos2> {
    let n = poly.len();
    if radius <= 0.0 || n < 3 {
        return poly.to_vec();
    }
    let mut out: Vec<Pos2> = Vec::with_capacity(n * (CORNER_STEPS + 1));
    for i in 0..n {
        let (prev, cur, next) = (poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]);
        let (a, b) = (prev - cur, next - cur);
        let r = radius.min(a.length() / 2.0).min(b.length() / 2.0);
        let (p0, p1) = (cur + a.normalized() * r, cur + b.normalized() * r);
        for s in 0..=CORNER_STEPS {
            let t = s as f32 / CORNER_STEPS as f32;
            let p = (p0.to_vec2() * (1.0 - t) * (1.0 - t) + cur.to_vec2() * 2.0 * t * (1.0 - t) + p1.to_vec2() * t * t)
                .to_pos2();
            // corners meeting at the middle of a side share a point
            if out.last().is_none_or(|q| q.distance(p) > 0.01) {
                out.push(p);
            }
        }
    }
    if out.len() > 1 && out[0].distance(out[out.len() - 1]) <= 0.01 {
        out.pop();
    }
    out
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
    let look = palette.look;
    let border = |color: Color32| {
        if look.border_width > 0.0 {
            Stroke::new(look.border_width, color.gamma_multiply(look.key_opacity))
        } else {
            Stroke::NONE
        }
    };
    let mut tooltip = None;
    // keycode shown on each key, for the combos
    let mut centers: Vec<(u16, Pos2)> = Vec::new();

    for key in view.keys {
        let role = key.role();
        let poly: Vec<Pos2> = inset(key).corners().iter().map(|&c| tf.pos(c)).collect();
        let center = tf.pos(key.center());
        let angle = key.r.to_radians();
        let (w, h) = ((key.w - 2.0 * GAP) * tf.scale, (key.h - 2.0 * GAP) * tf.scale);

        let Some(KeyRole::Matrix { row, col }) = role else {
            // encoders: a simple knob
            painter.circle(
                center,
                w.min(h) / 2.0,
                palette.key_empty.gamma_multiply(look.key_opacity),
                border(palette.key_border),
            );
            continue;
        };

        let own = view.keymap.get(view.layer, row, col);
        let (code, from) = view.keymap.resolve(view.layer, view.active_mask, row, col);
        centers.push((code, center));
        let overridden = (view.key_override)(code);
        let mut leg = match &overridden {
            Some((replacement, why)) => {
                let mut l = legend(*replacement, view.legends);
                l.tooltip = format!("{}\n{why}", l.tooltip);
                l
            }
            None => {
                let mut l = legend(code, view.legends);
                view.labels.apply(&mut l, view.layer, row, col, code, from);
                l
            }
        };
        if overridden.is_some() {
            leg.kind = LegendKind::Normal;
        }
        let transparent = own == KC_TRNS;
        let glow = (view.highlight)(row, col).clamp(0.0, 1.0);

        let mut fill = match leg.kind {
            LegendKind::Empty => palette.key_empty,
            LegendKind::Layer(l) => (view.layer_color)(l).map(|c| mix(palette.key, c, 0.5)).unwrap_or(palette.key),
            _ => palette.key,
        };
        if transparent {
            fill = mix(fill, palette.key_empty, 0.5);
        }
        let heat = view.heat.map(|f| f(row, col));
        let mut base_text = match () {
            _ if overridden.is_some() => palette.accent,
            _ if transparent => palette.text_dim,
            _ => palette.text,
        };
        if let Some((t, n)) = heat {
            fill = if n == 0 { palette.key_empty } else { heat_color(t) };
            base_text = if n == 0 { palette.text_dim } else { text_on(fill) };
        }
        fill = mix(fill, palette.pressed, 0.7 * glow);
        // see-through keys: a pressed key turns opaque, to stand out
        fill = fill.gamma_multiply(look.key_opacity + (1.0 - look.key_opacity) * glow);
        let text_color = mix(base_text, palette.pressed_text, glow);
        let mark = (view.mark)(row, col).clamp(0.0, 1.0);
        let stroke = if mark > 0.0 {
            Stroke::new(1.5 + 2.5 * mark, mix(palette.key_border, palette.accent, 0.5 + 0.5 * mark))
        } else {
            border(palette.key_border)
        };
        let shape = rounded_polygon(&poly, look.key_radius * w.min(h));
        painter.add(Shape::convex_polygon(shape, fill, stroke));

        let base = tf.scale * 0.30 * look.legend_scale;
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
            let mut tip = format!("{}{source}\nriga {row}, colonna {col}", leg.tooltip);
            if let Some((_, n)) = heat {
                tip.push_str(&format!("\n{n} pressioni"));
            }
            tooltip = Some(tip);
        }
    }
    if let Some(combos) = view.combos {
        if let Some(tip) = draw_combos(painter, combos, &centers, view, palette, tf.scale, hover) {
            tooltip = Some(tip);
        }
    }
    tooltip
}

/// Each combo whose keys are all on the layer shown: a badge with the output
/// between its keys, linked to them. Returns the tooltip of the hovered badge.
fn draw_combos(
    painter: &Painter,
    combos: &[Combo],
    centers: &[(u16, Pos2)],
    view: &KeyboardView,
    palette: &Palette,
    unit: f32,
    hover: Option<Pos2>,
) -> Option<String> {
    let mut tooltip = None;
    for combo in combos {
        let keys: Option<Vec<(u16, Pos2)>> =
            combo.inputs().map(|kc| centers.iter().find(|(c, _)| *c == kc).copied()).collect();
        let Some(keys) = keys else { continue };
        let mid = keys.iter().fold(Vec2::ZERO, |acc, (_, p)| acc + p.to_vec2()) / keys.len() as f32;
        let mid = mid.to_pos2();
        // keys far apart: lines from the badge to the edge of each key
        let line = Stroke::new(1.5, palette.accent.gamma_multiply(0.8));
        for (_, p) in &keys {
            let d = mid - *p;
            if d.length() > unit * 0.8 {
                let edge = *p + d.normalized() * unit * 0.45;
                painter.line_segment([mid, edge], line);
                painter.circle_filled(edge, 2.5, palette.accent);
            }
        }
        let out = legend(combo.output, view.legends);
        let size = unit * 0.18;
        let font = FontId::proportional(size);
        let text =
            if painter.fonts_mut(|f| f.has_glyphs(&font, &out.tap.symbol)) { out.tap.symbol } else { out.tap.text };
        let galley = painter.layout_no_wrap(text, font, text_on(palette.accent));
        let badge = Rect::from_center_size(mid, galley.size() + Vec2::new(size * 0.8, size * 0.35));
        painter.rect(
            badge,
            size * 0.4,
            palette.accent,
            Stroke::new(1.0, palette.background),
            egui::StrokeKind::Outside,
        );
        painter.galley(badge.center() - galley.size() / 2.0, galley, text_on(palette.accent));
        if hover.is_some_and(|p| badge.contains(p)) {
            let names: Vec<String> = keys.iter().map(|(c, _)| legend(*c, view.legends).tap.text).collect();
            tooltip = Some(format!("Combo: {} insieme → {}", names.join(" + "), out.tooltip));
        }
    }
    tooltip
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convex(poly: &[Pos2]) -> bool {
        let n = poly.len();
        let cross = |i: usize| {
            let (a, b, c) = (poly[i], poly[(i + 1) % n], poly[(i + 2) % n]);
            (b - a).x * (c - b).y - (b - a).y * (c - b).x
        };
        let first = cross(0).signum();
        (0..n).all(|i| cross(i).abs() < 1e-3 || cross(i).signum() == first)
    }

    #[test]
    fn rounded_corners() {
        let sq = [Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0), Pos2::new(10.0, 10.0), Pos2::new(0.0, 10.0)];
        assert_eq!(rounded_polygon(&sq, 0.0), sq.to_vec());
        let r = rounded_polygon(&sq, 2.0);
        assert_eq!(r.len(), 4 * (CORNER_STEPS + 1));
        assert!(convex(&r));
        assert!(r.iter().all(|&p| contains(&sq, p)));
        // the corner itself is cut
        assert!(r.iter().all(|p| p.distance(Pos2::ZERO) > 0.5));
        // a radius larger than half a side: corners meet in the middle, no duplicates
        let big = rounded_polygon(&sq, 50.0);
        assert!(big.windows(2).all(|w| w[0].distance(w[1]) > 0.01));
        assert!(big.iter().any(|p| p.distance(Pos2::new(5.0, 0.0)) < 0.01));
        // rotated key
        let rot: Vec<Pos2> = sq.iter().map(|p| (rotate(p.to_vec2(), 0.5) + Vec2::new(20.0, 5.0)).to_pos2()).collect();
        let r = rounded_polygon(&rot, 3.0);
        assert!(convex(&r));
        // points on the sides: nudged towards the center against rounding errors
        let c = (rot.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2()) / 4.0).to_pos2();
        assert!(r.iter().all(|&p| contains(&rot, p + (c - p) * 1e-3)));
    }

    #[test]
    fn point_in_convex_polygon() {
        let sq = [Pos2::new(0.0, 0.0), Pos2::new(2.0, 0.0), Pos2::new(2.0, 2.0), Pos2::new(0.0, 2.0)];
        assert!(contains(&sq, Pos2::new(1.0, 1.0)));
        assert!(!contains(&sq, Pos2::new(3.0, 1.0)));
        assert!(!contains(&sq, Pos2::new(-0.1, 1.0)));
    }
}
