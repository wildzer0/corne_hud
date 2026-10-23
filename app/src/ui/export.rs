//! Keymap export: a printable HTML page with one SVG drawing per layer,
//! plus combos, tap dances, macros and key overrides in words. The SVG of
//! each layer is drawn like the HUD draws the keyboard, in the light theme.

use super::keyboard::{inset, rounded_polygon};
use super::theme::{mix, Palette};
use crate::changes;
use crate::kle::{KeyRole, KleKey};
use crate::labels::Labels;
use crate::model::Keymap;
use crate::tr_format;
use crate::vial::definition::CustomKeycode;
use crate::vial::dynamic::Dynamic;
use crate::vial::keycode::{KC_NO, KC_TRNS};
use crate::vial::legend::{legend, LegendContext, LegendKind};
use egui::{Color32, Pos2};
use std::fmt::Write as _;

/// SVG pixels per key unit.
const UNIT: f32 = 54.0;
const MARGIN: f32 = 6.0;
const MACRO_BASE: u16 = 0x7700;

pub struct ExportInput<'a> {
    pub keyboard: &'a str,
    /// When it was made, shown under the title.
    pub date: &'a str,
    pub keys: &'a [KleKey],
    pub keymap: &'a Keymap,
    pub dynamic: &'a Dynamic,
    pub layer_names: &'a [String],
    pub layer_colors: &'a [Option<Color32>],
    pub custom: &'a [CustomKeycode],
    pub labels: &'a Labels,
    pub palette: &'a Palette,
}

impl ExportInput<'_> {
    fn legends(&self) -> LegendContext<'_> {
        LegendContext {
            layer_names: self.layer_names,
            custom: self.custom,
            tap_dances: &self.dynamic.tap_dances,
            macros: &self.dynamic.macros,
        }
    }

    fn layer_name(&self, l: u8) -> String {
        self.layer_names.get(l as usize).cloned().unwrap_or_else(|| format!("L{l}"))
    }

    fn layer_color(&self, l: u8) -> Option<Color32> {
        self.layer_colors.get(l as usize).copied().flatten()
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn css(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// Font size for `text` to fit `width`, never above `size`.
fn fitted(text: &str, size: f32, width: f32) -> f32 {
    // average glyph about 0.6 em; lone symbols a little larger, like on the HUD
    let n = text.chars().count().max(1) as f32;
    let lone = n == 1.0 && !text.chars().all(char::is_alphanumeric);
    let size = if lone { size * 1.3 } else { size };
    size.min(width / (n * 0.6)).max(size * 0.45)
}

/// One layer as an SVG drawing.
pub fn layer_svg(input: &ExportInput, layer: u8) -> String {
    let (mut min, mut max) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for k in input.keys {
        for (x, y) in k.corners() {
            min = (min.0.min(x), min.1.min(y));
            max = (max.0.max(x), max.1.max(y));
        }
    }
    if min.0 > max.0 {
        return String::new();
    }
    let pos = |(x, y): (f32, f32)| Pos2::new((x - min.0) * UNIT + MARGIN, (y - min.1) * UNIT + MARGIN);
    let (w, h) = ((max.0 - min.0) * UNIT + 2.0 * MARGIN, (max.1 - min.1) * UNIT + 2.0 * MARGIN);
    let p = input.palette;
    let look = p.look;
    let legends = input.legends();
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.0} {h:.0}" width="{w:.0}" height="{h:.0}" font-family="DejaVu Sans, Segoe UI, sans-serif">"#
    );
    for key in input.keys {
        let poly: Vec<Pos2> = inset(key).corners().iter().map(|&c| pos(c)).collect();
        let center = pos(key.center());
        let (kw, kh) = ((key.w - 0.12) * UNIT, (key.h - 0.12) * UNIT);
        let points = |pts: &[Pos2]| pts.iter().map(|q| format!("{:.1},{:.1}", q.x, q.y)).collect::<Vec<_>>().join(" ");
        let stroke = if look.border_width > 0.0 { css(p.key_border) } else { "none".into() };
        let Some(KeyRole::Matrix { row, col }) = key.role() else {
            let _ = write!(
                svg,
                r#"<circle cx="{:.1}" cy="{:.1}" r="{:.1}" fill="{}" stroke="{stroke}"/>"#,
                center.x,
                center.y,
                kw.min(kh) / 2.0,
                css(p.key_empty)
            );
            continue;
        };
        let own = input.keymap.get(layer, row, col);
        let (code, from) = input.keymap.resolve(layer, (1 << layer) | 1, row, col);
        let mut leg = legend(code, &legends);
        input.labels.apply(&mut leg, layer, row, col, code, from);
        let transparent = own == KC_TRNS;
        let mut fill = match leg.kind {
            LegendKind::Empty => p.key_empty,
            LegendKind::Layer(l) => input.layer_color(l).map_or(p.key, |c| mix(p.key, c, 0.5)),
            _ => p.key,
        };
        if transparent {
            fill = mix(fill, p.key_empty, 0.5);
        }
        let color = css(if transparent { p.text_dim } else { p.text });
        let shape = rounded_polygon(&poly, look.key_radius * kw.min(kh));
        let _ = write!(
            svg,
            r#"<polygon points="{}" fill="{}" stroke="{stroke}" stroke-width="{:.1}"><title>{}</title></polygon>"#,
            points(&shape),
            css(fill),
            look.border_width.max(1.0),
            escape(&leg.tooltip)
        );
        let base = UNIT * 0.30 * look.legend_scale;
        let text = |s: &str, dy: f32, size: f32, opacity: f32| {
            if s.is_empty() {
                return String::new();
            }
            let size = fitted(s, size, kw * 0.88);
            format!(
                r#"<text x="{:.1}" y="{:.1}" font-size="{size:.1}" fill="{color}" fill-opacity="{opacity}" text-anchor="middle" dominant-baseline="central" transform="rotate({:.1} {:.1} {:.1})">{}</text>"#,
                center.x,
                center.y + dy,
                key.r,
                center.x,
                center.y,
                escape(s)
            )
        };
        let shown = |t: &crate::vial::legend::Text| if t.symbol.is_empty() { t.text.clone() } else { t.symbol.clone() };
        match &leg.hold {
            Some(hold) => {
                svg.push_str(&text(&shown(&leg.tap), -kh * 0.12, base, 1.0));
                svg.push_str(&text(&shown(hold), kh * 0.28, base * 0.55, 0.75));
            }
            None => svg.push_str(&text(&shown(&leg.tap), 0.0, base, 1.0)),
        }
    }
    svg.push_str("</svg>");
    svg
}

/// A layer worth printing: at least one key that is not empty or transparent.
fn has_keys(input: &ExportInput, layer: u8) -> bool {
    input.keys.iter().any(|k| match k.role() {
        Some(KeyRole::Matrix { row, col }) => !matches!(input.keymap.get(layer, row, col), KC_NO | KC_TRNS),
        _ => false,
    })
}

/// The whole page.
pub fn html(input: &ExportInput) -> String {
    let legends = input.legends();
    let title = escape(input.keyboard);
    let mut out = format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><title>{title} · keymap</title>
<style>
body {{ font-family: "Segoe UI", system-ui, sans-serif; margin: 24px; color: #222; background: #fff; }}
h1 {{ margin: 0 0 4px; font-size: 22px; }}
.date {{ color: #777; margin-bottom: 20px; }}
section {{ break-inside: avoid; margin-bottom: 22px; }}
h2 {{ font-size: 16px; margin: 0 0 6px; display: flex; align-items: center; gap: 8px; }}
.swatch {{ width: 12px; height: 12px; border-radius: 3px; display: inline-block; }}
svg {{ max-width: 100%; height: auto; }}
ul {{ margin: 4px 0; padding-left: 20px; }}
li {{ margin: 2px 0; }}
@media print {{ body {{ margin: 0; }} }}
</style></head><body>
<h1>{title}</h1>
<div class="date">{}</div>
"#,
        escape(input.date)
    );
    for layer in (0..input.keymap.layers()).filter(|&l| has_keys(input, l)) {
        let swatch = input
            .layer_color(layer)
            .map(|c| format!(r#"<span class="swatch" style="background:{}"></span>"#, css(c)))
            .unwrap_or_default();
        let _ = write!(
            out,
            "<section><h2>{swatch}{} <small style=\"color:#888;font-weight:normal\">({})</small></h2>\n{}\n</section>\n",
            escape(&input.layer_name(layer)),
            tr_format!("layer {layer}", "layer {layer}"),
            layer_svg(input, layer)
        );
    }
    let mut list = |title: String, items: Vec<String>| {
        if items.is_empty() {
            return;
        }
        let _ = write!(out, "<section><h2>{}</h2><ul>", escape(&title));
        for i in items {
            let _ = write!(out, "<li>{}</li>", escape(&i));
        }
        out.push_str("</ul></section>\n");
    };
    let d = input.dynamic;
    list("Combo".into(), d.combos.iter().filter(|c| c.is_active()).map(|c| changes::combo_text(c, &legends)).collect());
    list(
        "Tap dance".into(),
        d.tap_dances
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.is_empty())
            .map(|(i, t)| format!("TD({i}): {}", changes::tap_dance_text(t, &legends)))
            .collect(),
    );
    list(
        "Macro".into(),
        d.macros
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.is_empty())
            .map(|(i, _)| {
                let tip = legend(MACRO_BASE + i as u16, &legends).tooltip.replace('\n', " · ");
                format!("M{i}: {tip}")
            })
            .collect(),
    );
    let names = changes::Context { layer_names: input.layer_names, custom: input.custom };
    list(
        "Key override".into(),
        d.key_overrides
            .iter()
            .filter(|o| o.trigger != 0 || o.replacement != 0)
            .map(|o| changes::override_text(o, &legends, &names))
            .collect(),
    );
    out.push_str("</body></html>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::mock::{mock_dynamic, MOCK_VIAL_JSON, MOCK_VIL};
    use crate::ui::theme::Palette;
    use crate::vial::definition::Definition;
    use crate::vial::vil::Vil;

    #[test]
    fn page_with_every_layer_and_list() {
        let def = Definition::from_json_str(MOCK_VIAL_JSON).unwrap();
        let keys = def.visible_keys(0);
        let keymap = Vil::parse(MOCK_VIL, def.rows, def.cols).unwrap().keymap;
        let dynamic = mock_dynamic();
        let names: Vec<String> = ["BASE", "SYM", "NAV", "SYS", "DEBUG", "ACC"].map(String::from).to_vec();
        let colors = vec![None, Some(Color32::from_rgb(0, 150, 200)), None, None, None, None];
        let (labels, _) = Labels::from_config(&[]);
        let palette = Palette::light(1.0);
        let input = ExportInput {
            keyboard: "Corne <v4>",
            date: "2026-10-23",
            keys: &keys,
            keymap: &keymap,
            dynamic: &dynamic,
            layer_names: &names,
            layer_colors: &colors,
            custom: &def.custom,
            labels: &labels,
            palette: &palette,
        };
        let page = html(&input);
        assert!(page.starts_with("<!doctype html>"));
        // escaped title, every non-empty layer drawn once
        assert!(page.contains("Corne &lt;v4&gt;"));
        assert_eq!(page.matches("<svg").count(), (0..6).filter(|&l| has_keys(&input, l)).count());
        assert!(page.contains("<h2>Combo</h2>") && page.contains("<h2>Key override</h2>"));
        // one shape per key, the layer color on SYM's swatch
        let svg = layer_svg(&input, 0);
        assert_eq!(svg.matches("<polygon").count() + svg.matches("<circle").count(), keys.len());
        assert!(page.contains("background:#0096c8"));
        assert_eq!(fitted("Esc", 16.0, 100.0), 16.0);
        assert!(fitted("PrintScreen", 16.0, 40.0) < 16.0);
    }
}
