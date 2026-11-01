//! Composants visuels : potards, interrupteurs, courbe d'EQ, courbe du compresseur.

use crate::state::{Channel, Kind};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Id, Pos2, Rect, Response, Sense, Shape, Stroke, Ui};
use std::f32::consts::PI;

pub const BG: Color32 = Color32::from_rgb(0x16, 0x18, 0x1D);
pub const CARD: Color32 = Color32::from_rgb(0x20, 0x23, 0x2A);
pub const TRACK: Color32 = Color32::from_rgb(0x33, 0x37, 0x40);
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE8, 0xEC);
pub const DIM: Color32 = Color32::from_rgb(0x8A, 0x90, 0x9A);
pub const OFF: Color32 = Color32::from_rgb(0x55, 0x5A, 0x63);

#[derive(Clone, Copy)]
pub enum Scale {
    Lin,
    Log,
}

pub struct Knob<'a> {
    pub label: &'a str,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub step: f64,
    pub scale: Scale,
    pub bipolar: bool,
    pub format: &'a dyn Fn(f64) -> String,
    pub accent: Color32,
    pub enabled: bool,
}

impl Knob<'_> {
    fn to_t(&self, v: f64) -> f64 {
        match self.scale {
            Scale::Lin => (v - self.min) / (self.max - self.min),
            Scale::Log => (v / self.min).ln() / (self.max / self.min).ln(),
        }
        .clamp(0.0, 1.0)
    }
    fn from_t(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        let v = match self.scale {
            Scale::Lin => self.min + t * (self.max - self.min),
            Scale::Log => self.min * (self.max / self.min).powf(t),
        };
        ((v / self.step).round() * self.step).clamp(self.min, self.max)
    }

    /// Interprète une saisie clavier : « 3,5k », « 120 Hz », « -12 », « inf », « G 40 », « centre »…
    fn parse(&self, input: &str) -> Option<f64> {
        let s = input.trim().to_lowercase().replace(',', ".").replace(' ', "");
        if s.is_empty() {
            return None;
        }
        let is_pan = (self.format)(0.0) == "Centre";
        let v = if s.contains("inf") || s.contains('∞') {
            if s.starts_with('-') { self.min } else { self.max }
        } else if is_pan && (s.starts_with('c')) {
            0.0
        } else {
            let num: String = s.chars().filter(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+')).collect();
            let mut n: f64 = num.parse().ok()?;
            if s.contains('k') {
                n *= 1000.0;
            }
            if is_pan {
                n = n.abs().min(100.0) / 100.0 * if s.starts_with('g') || s.starts_with('-') { -1.0 } else { 1.0 };
            }
            n
        };
        Some(((v / self.step).round() * self.step).clamp(self.min, self.max))
    }

    pub fn show(self, ui: &mut Ui, value: &mut f64) -> Response {
        let d = 46.0;
        let (rect, area) = ui.allocate_exact_size(vec2(64.0, d + 34.0), Sense::hover());
        // Zone du potard (cercle + libellé) et zone de la valeur, séparées pour que la saisie
        // au clavier ne se batte pas avec le glisser du potard.
        let knob_rect = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + d + 15.0));
        let text_rect = Rect::from_center_size(pos2(rect.center().x, rect.top() + d + 24.0), vec2(64.0, 18.0));
        let mut resp = ui.interact(knob_rect, area.id.with("knob"), Sense::click_and_drag());
        let text_resp = ui.interact(text_rect, area.id.with("value"), Sense::click());
        let id = resp.id.with("raw");
        if resp.drag_started() {
            ui.data_mut(|m| m.insert_temp(id, self.to_t(*value)));
        }
        let before = *value;
        if resp.dragged() {
            let fine = if ui.input(|i| i.modifiers.shift) { 0.2 } else { 1.0 };
            let dl = resp.drag_delta();
            let mut t: f64 = ui.data(|m| m.get_temp(id)).unwrap_or(self.to_t(*value));
            t = (t + ((dl.x - dl.y) as f64) * 0.004 * fine).clamp(0.0, 1.0);
            ui.data_mut(|m| m.insert_temp(id, t));
            *value = self.from_t(t);
        }
        if resp.hovered() {
            let s = ui.input(|i| i.smooth_scroll_delta.y) as f64;
            if s != 0.0 {
                let t = self.to_t(*value) + s * 0.0006;
                let nv = self.from_t(t);
                // garantit au moins un cran par mouvement de molette
                *value = if nv == *value { (*value + self.step * s.signum()).clamp(self.min, self.max) } else { nv };
            }
        }
        // Saisie au clavier : clic sur la valeur affichée sous le potard
        let edit_id = resp.id.with("edit");
        let click_on_text = text_resp.clicked();
        if click_on_text {
            ui.data_mut(|m| m.insert_temp(edit_id, ((self.format)(*value), 0u8)));
        }
        let mut editing: Option<(String, u8)> = ui.data(|m| m.get_temp(edit_id));
        if let Some((text, frames)) = editing.as_mut() {
            let te_id = edit_id.with("te");
            let out = ui.put(
                text_rect,
                egui::TextEdit::singleline(text)
                    .id(te_id)
                    .font(FontId::proportional(13.0))
                    .horizontal_align(egui::Align::Center)
                    .margin(vec2(2.0, 1.0)),
            );
            // Le clic sur le potard reprend le focus : on le redemande pendant quelques images
            if *frames < 3 {
                out.request_focus();
                if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), te_id) {
                    let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
                    st.cursor.set_char_range(Some(all));
                    st.store(ui.ctx(), te_id);
                }
                ui.ctx().request_repaint();
            }
            *frames = frames.saturating_add(1);
            let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
            let focused = ui.memory(|m| m.has_focus(te_id));
            if *frames > 3 && !focused {
                if !escape {
                    if let Some(v) = self.parse(text) {
                        *value = v;
                    }
                }
                ui.data_mut(|m| m.remove::<(String, u8)>(edit_id));
                editing = None;
            } else {
                let keep = (text.clone(), *frames);
                ui.data_mut(|m| m.insert_temp(edit_id, keep));
            }
        }
        if resp.double_clicked() && !click_on_text {
            *value = self.default;
        }
        if *value != before {
            resp.mark_changed();
        }

        let p = ui.painter();
        let c = pos2(rect.center().x, rect.top() + d / 2.0 + 2.0);
        let r = d / 2.0 - 3.0;
        let accent = if self.enabled { self.accent } else { OFF };
        let a0 = 0.75 * PI;
        let span = 1.5 * PI;
        let arc = |from: f32, to: f32| -> Vec<Pos2> {
            let n = 40;
            (0..=n).map(|i| {
                let a = from + (to - from) * i as f32 / n as f32;
                c + vec2(a.cos(), a.sin()) * r
            }).collect()
        };
        p.circle_filled(c, r - 5.0, Color32::from_rgb(0x2A, 0x2E, 0x36));
        p.add(Shape::line(arc(a0, a0 + span), Stroke::new(4.0_f32, TRACK)));
        let t = self.to_t(*value) as f32;
        let at = a0 + span * t;
        let start = if self.bipolar { a0 + span * self.to_t(0.0) as f32 } else { a0 };
        if (at - start).abs() > 0.01 {
            p.add(Shape::line(arc(start.min(at), start.max(at)), Stroke::new(4.0_f32, accent)));
        }
        p.line_segment([c + vec2(at.cos(), at.sin()) * (r - 14.0), c + vec2(at.cos(), at.sin()) * (r - 5.0)], Stroke::new(2.5_f32, TEXT));
        if resp.hovered() || resp.dragged() {
            p.circle_stroke(c, r + 2.0, Stroke::new(1.0_f32, accent.gamma_multiply(0.6)));
        }
        p.text(pos2(rect.center().x, rect.top() + d + 8.0), Align2::CENTER_CENTER, self.label, FontId::proportional(12.0), DIM);
        if editing.is_none() {
            let hover_text = text_resp.hovered();
            if hover_text {
                p.rect_stroke(text_rect.shrink2(vec2(4.0, 0.0)), 4.0, Stroke::new(1.0_f32, accent.gamma_multiply(0.5)));
            }
            p.text(
                text_rect.center(),
                Align2::CENTER_CENTER,
                (self.format)(*value),
                FontId::proportional(13.0),
                if self.enabled { TEXT } else { DIM },
            );
        }
        resp.on_hover_text("Glisser pour régler (Maj = fin), molette, double-clic = valeur par défaut, clic sur la valeur = saisie au clavier")
    }
}

/// Interrupteur en forme de pastille, avec libellé.
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str, accent: Color32) -> Response {
    let font = FontId::proportional(14.0);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, TEXT);
    let w = 34.0 + 8.0 + galley.size().x;
    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let p = ui.painter();
    let pill = Rect::from_min_size(pos2(rect.left(), rect.center().y - 9.0), vec2(34.0, 18.0));
    p.rect_filled(pill, 9.0, if *on { accent } else { TRACK });
    let kx = if *on { pill.right() - 9.0 } else { pill.left() + 9.0 };
    p.circle_filled(pos2(kx, pill.center().y), 7.0, TEXT);
    p.galley(pos2(pill.right() + 8.0, rect.center().y - galley.size().y / 2.0), galley, TEXT);
    resp
}

const F_MIN: f64 = 20.0;
const F_MAX: f64 = 20000.0;
const DB: f64 = 18.0;

/// Courbe d'EQ interactive. Les points 1 à 4 se déplacent à la souris (molette = largeur Q),
/// le point H règle le coupe-bas.
pub fn eq_graph(ui: &mut Ui, ch: &mut Channel, accent: Color32, height: f32) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, height), Sense::click_and_drag());
    let plot = rect.shrink2(vec2(28.0, 10.0)).translate(vec2(10.0, -4.0));
    let fx = |f: f64| plot.left() + ((f / F_MIN).ln() / (F_MAX / F_MIN).ln()) as f32 * plot.width();
    let xf = |x: f32| F_MIN * (F_MAX / F_MIN).powf(((x - plot.left()) / plot.width()).clamp(0.0, 1.0) as f64);
    let gy = |g: f64| plot.center().y - (g / DB) as f32 * plot.height() / 2.0;
    let yg = |y: f32| (-(y - plot.center().y) / (plot.height() / 2.0)) as f64 * DB;

    let p = ui.painter_at(rect);
    p.rect_filled(rect, 8.0, Color32::from_rgb(0x1A, 0x1D, 0x23));
    for f in [50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0] {
        let x = fx(f);
        p.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(0x2A, 0x2E, 0x36)));
        let t = if f >= 1000.0 { format!("{}k", f / 1000.0) } else { format!("{f}") };
        p.text(pos2(x, plot.bottom() + 8.0), Align2::CENTER_CENTER, t, FontId::proportional(10.0), DIM);
    }
    for g in [-12.0, -6.0, 0.0, 6.0, 12.0] {
        let y = gy(g);
        let col = if g == 0.0 { Color32::from_rgb(0x44, 0x49, 0x52) } else { Color32::from_rgb(0x2A, 0x2E, 0x36) };
        p.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], Stroke::new(1.0_f32, col));
        p.text(pos2(plot.left() - 6.0, y), Align2::RIGHT_CENTER, format!("{g:+}"), FontId::proportional(10.0), DIM);
    }

    let active = ch.eq_on || ch.hpf_on;
    let col = if active { accent } else { OFF };
    let n = 240;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let x = plot.left() + plot.width() * i as f32 / n as f32;
            pos2(x, gy(ch.response_db(xf(x)).clamp(-DB * 1.4, DB * 1.4)).clamp(plot.top(), plot.bottom()))
        })
        .collect();
    let zero = gy(0.0);
    for w2 in pts.windows(2) {
        let quad = vec![w2[0], w2[1], pos2(w2[1].x, zero), pos2(w2[0].x, zero)];
        p.add(Shape::convex_polygon(quad, col.gamma_multiply(0.12), Stroke::NONE));
    }
    p.add(Shape::line(pts, Stroke::new(2.5_f32, col)));

    // Points
    let handles: Vec<Pos2> = ch
        .bands
        .iter()
        .map(|b| pos2(fx(b.freq), gy(b.gain.clamp(-DB, DB))))
        .chain(std::iter::once(pos2(fx(ch.hpf_freq), gy(ch.response_db(ch.hpf_freq).clamp(-DB, DB)).min(plot.bottom() - 8.0))))
        .collect();
    let drag_id = Id::new(rect.left_top().x as i32).with("eq-drag").with(accent.r());
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    let nearest = pointer.and_then(|pp| {
        handles.iter().enumerate().map(|(i, h)| (i, h.distance(pp))).filter(|(_, d)| *d < 16.0)
            .min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i)
    });
    if resp.drag_started() {
        ui.data_mut(|m| m.insert_temp(drag_id, nearest));
    }
    let dragging: Option<usize> = if resp.dragged() { ui.data(|m| m.get_temp(drag_id)).flatten() } else { None };
    let mut changed = false;
    if let (Some(i), Some(pp)) = (dragging, resp.interact_pointer_pos()) {
        let f = xf(pp.x).round().clamp(20.0, 20000.0);
        if i < 4 {
            let b = &mut ch.bands[i];
            b.freq = f;
            b.gain = ((yg(pp.y) * 2.0).round() / 2.0).clamp(-DB, DB);
        } else {
            ch.hpf_freq = f.clamp(20.0, 500.0);
        }
        changed = true;
    }
    let hovered = dragging.or(nearest);
    if let Some(i) = hovered.filter(|&i| i < 4) {
        let s = ui.input(|inp| inp.smooth_scroll_delta.y) as f64;
        if s != 0.0 && resp.hovered() {
            let b = &mut ch.bands[i];
            b.q = ((b.q * (s * 0.004).exp() * 100.0).round() / 100.0).clamp(0.1, 16.0);
            changed = true;
        }
        if resp.double_clicked() {
            ch.bands[i].gain = 0.0;
            changed = true;
        }
    }
    for (i, h) in handles.iter().enumerate() {
        let on = if i < 4 { ch.eq_on } else { ch.hpf_on };
        let fill = if on { accent } else { OFF };
        let r = if hovered == Some(i) { 10.0 } else { 8.0 };
        p.circle_filled(*h, r, fill);
        p.circle_stroke(*h, r, Stroke::new(1.5_f32, Color32::from_rgb(0x10, 0x12, 0x16)));
        let t = if i < 4 { format!("{}", i + 1) } else { "H".into() };
        p.text(*h, Align2::CENTER_CENTER, t, FontId::proportional(11.0), Color32::from_rgb(0x10, 0x12, 0x16));
    }
    if let Some(i) = hovered {
        let tip = if i < 4 {
            let b = &ch.bands[i];
            format!("Bande {} · {} · {} · {:+.1} dB · Q {:.2}\nGlisser = fréquence et gain, molette = Q, double-clic = 0 dB",
                i + 1, b.kind.label(), fmt_hz(b.freq), b.gain, b.q)
        } else {
            format!("Coupe-bas · {}\nGlisser horizontalement", fmt_hz(ch.hpf_freq))
        };
        resp.on_hover_text(tip);
    }
    changed
}

/// Courbe entrée / sortie du compresseur.
pub fn comp_graph(ui: &mut Ui, ch: &Channel, accent: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6.0, Color32::from_rgb(0x1A, 0x1D, 0x23));
    let pr = rect.shrink(6.0);
    let map = |i: f64, o: f64| {
        pos2(pr.left() + ((i + 60.0) / 60.0) as f32 * pr.width(), pr.bottom() - ((o + 60.0) / 72.0) as f32 * pr.height())
    };
    p.line_segment([map(-60.0, -60.0), map(0.0, 0.0)], Stroke::new(1.0_f32, Color32::from_rgb(0x33, 0x37, 0x40)));
    let col = if ch.comp_on { accent } else { OFF };
    if ch.comp_on {
        let t = map(ch.thresh, -60.0).x;
        p.line_segment([pos2(t, pr.top()), pos2(t, pr.bottom())], Stroke::new(1.0_f32, accent.gamma_multiply(0.35)));
    }
    let pts: Vec<Pos2> = (0..=60).map(|k| {
        let i = -60.0 + k as f64;
        let q = map(i, ch.comp_curve(i).clamp(-60.0, 12.0));
        pos2(q.x, q.y.clamp(pr.top(), pr.bottom()))
    }).collect();
    p.add(Shape::line(pts, Stroke::new(2.0_f32, col)));
}

pub fn fmt_hz(f: f64) -> String {
    if f >= 1000.0 { format!("{:.2} kHz", f / 1000.0).replace(".00", "").replace('.', ",") } else { format!("{f:.0} Hz") }
}

pub fn kind_choices(band: usize) -> &'static [Kind] {
    match band {
        0 => &[Kind::Cloche, Kind::PlateauGrave],
        3 => &[Kind::Cloche, Kind::PlateauAigu],
        _ => &[Kind::Cloche],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn knob<'a>(min: f64, max: f64, step: f64, format: &'a dyn Fn(f64) -> String) -> Knob<'a> {
        Knob { label: "", min, max, default: 0.0, step, scale: Scale::Lin, bipolar: false, format, accent: TEXT, enabled: true }
    }

    #[test]
    fn typed_values_are_understood() {
        let f = |v: f64| format!("{v}");
        let freq = knob(20.0, 20000.0, 1.0, &f);
        assert_eq!(freq.parse("3,5k"), Some(3500.0));
        assert_eq!(freq.parse("3.5 kHz"), Some(3500.0));
        assert_eq!(freq.parse("120 Hz"), Some(120.0));
        assert_eq!(freq.parse("99999"), Some(20000.0));
        assert_eq!(freq.parse("abc"), None);
        let gain = knob(-18.0, 18.0, 0.5, &f);
        assert_eq!(gain.parse("-2,3 dB"), Some(-2.5));
        assert_eq!(gain.parse("+4"), Some(4.0));
        let ratio = knob(1.0, 21.0, 0.1, &f);
        assert_eq!(ratio.parse("∞"), Some(21.0));
        assert_eq!(ratio.parse("inf"), Some(21.0));
        let fader = knob(-60.0, 6.0, 0.5, &f);
        assert_eq!(fader.parse("-inf"), Some(-60.0));
        let pf = |v: f64| if v == 0.0 { "Centre".to_string() } else { format!("{v}") };
        let pan = knob(-1.0, 1.0, 0.01, &pf);
        assert_eq!(pan.parse("G 40"), Some(-0.4));
        assert_eq!(pan.parse("d40 %"), Some(0.4));
        assert_eq!(pan.parse("-25"), Some(-0.25));
        assert_eq!(pan.parse("centre"), Some(0.0));
    }
}
