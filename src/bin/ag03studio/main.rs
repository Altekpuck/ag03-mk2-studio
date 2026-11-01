//! AG03 MK2 Studio : réglage complet de la Yamaha AG03 MK2 en une seule vue.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod midi;
mod profiles;
mod state;
mod widgets;

use eframe::egui::{self, vec2, Color32, RichText};
use profiles::Profile;
use state::{Channel, Mix, AMP_MODELS, RATIO_INF, REVERBS};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use widgets::*;

const CH1: Color32 = Color32::from_rgb(0xF2, 0xA1, 0x3B);
const CH2: Color32 = Color32::from_rgb(0x3F, 0xB6, 0xE8);
const REV: Color32 = Color32::from_rgb(0xA8, 0x7B, 0xF0);
const CELL: Color32 = Color32::from_rgb(0x1A, 0x1D, 0x23);

fn fr(s: String) -> String {
    s.replace('.', ",")
}

struct App {
    mix: Mix,
    midi: midi::Midi,
    last_sent: HashMap<Vec<u8>, Vec<u8>>,
    generation: u64,
    last_flush: Instant,
    builtin: Vec<Profile>,
    user: Vec<Profile>,
    save_dialog: Option<(usize, String)>,
    saved: Mix,
    last_save: Instant,
    screenshot: Option<(String, u32)>,
    logo: egui::TextureHandle,
    /// Hauteur de la case ampli du canal 2 : la courbe d'EQ du canal 1 grandit d'autant (cadres alignés).
    amp_h: f32,
    /// Ajustement automatique de la taille de la fenêtre au contenu (une fois, au démarrage).
    fit_frames: u32,
}

fn state_path() -> std::path::PathBuf {
    profiles::config_dir().join("etat.json")
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        setup_style(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        let mix: Mix = std::fs::read_to_string(state_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        App {
            saved: mix.clone(),
            mix,
            midi: midi::Midi::start(move || ctx.request_repaint()),
            last_sent: HashMap::new(),
            generation: 0,
            last_flush: Instant::now(),
            builtin: profiles::builtin(),
            user: profiles::load_user(),
            save_dialog: None,
            last_save: Instant::now(),
            screenshot: std::env::var("AG03_SCREENSHOT").ok().map(|p| (p, 0)),
            logo: cc.egui_ctx.load_texture(
                "logo",
                egui::ColorImage::from_rgba_unmultiplied([176, 155], include_bytes!("../../../assets/lopjoy.rgba")),
                egui::TextureOptions::LINEAR,
            ),
            amp_h: 0.0,
            fit_frames: 0,
        }
    }

    /// Envoie au boîtier uniquement ce qui a changé (au plus toutes les 25 ms).
    fn flush(&mut self, ctx: &egui::Context, connected: bool) {
        if !connected {
            return;
        }
        let bodies = self.mix.bodies();
        let changed: Vec<Vec<u8>> = bodies
            .into_iter()
            .filter(|b| self.last_sent.get(&state::key(b)) != Some(b))
            .collect();
        if changed.is_empty() {
            return;
        }
        if self.last_flush.elapsed() < Duration::from_millis(25) {
            ctx.request_repaint_after(Duration::from_millis(25));
            return;
        }
        for b in &changed {
            self.last_sent.insert(state::key(b), b.clone());
        }
        self.midi.send(changed);
        self.last_flush = Instant::now();
    }

    fn autosave(&mut self) {
        if self.mix != self.saved && self.last_save.elapsed() > Duration::from_secs(2) {
            if let Ok(s) = serde_json::to_string_pretty(&self.mix) {
                let _ = std::fs::write(state_path(), s);
            }
            self.saved = self.mix.clone();
            self.last_save = Instant::now();
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, st: &midi::Status) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(vec2(26.0, 23.0)));
            ui.label(RichText::new("AG03 MK2 Studio").size(20.0).strong().color(TEXT));
            ui.add_space(16.0);
            let (dot, txt) = if st.connected {
                (Color32::from_rgb(0x4C, 0xD1, 0x7A), format!("Connecté · {}", st.port))
            } else {
                (Color32::from_rgb(0xE5, 0x5B, 0x5B), format!("Non connecté · {}", if st.error.is_empty() { "recherche…" } else { &st.error }))
            };
            let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter().circle_filled(r.center(), 5.0, dot);
            ui.label(RichText::new(txt).color(TEXT));
            if st.connected {
                let acked = self.midi.acked.load(std::sync::atomic::Ordering::Relaxed);
                ui.label(RichText::new(format!("{} envoyés · {} confirmés", st.sent, acked)).color(DIM).size(12.0));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Tout renvoyer").on_hover_text("Renvoie tous les réglages au boîtier").clicked() {
                    self.last_sent.clear();
                }
                ui.add_space(8.0);
                let mut mute = self.mix.footswitch_mute;
                egui::ComboBox::from_id_salt("pedale")
                    .selected_text(if mute { "Coupe le canal 1" } else { "Reverb on/off" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut mute, false, "Reverb on/off");
                        ui.selectable_value(&mut mute, true, "Coupe le canal 1");
                    });
                self.mix.footswitch_mute = mute;
                ui.label(RichText::new("Pédale :").color(DIM));
            });
        });
    }

    fn channel(&mut self, ui: &mut egui::Ui, idx: usize) {
        let accent = if idx == 0 { CH1 } else { CH2 };
        let title = if idx == 0 { "Canal 1 · Micro" } else { "Canal 2 · Instrument / ligne" };
        card(ui, |ui| {
            self.profile_header(ui, idx, accent, title);
            let gap = 8.0;

            // Entrée : trois cases de même largeur, potards sur une même ligne
            let (pan, rev_on, rev) = if idx == 0 {
                (&mut self.mix.pan1, &mut self.mix.rev1_on, &mut self.mix.rev1)
            } else {
                (&mut self.mix.pan2, &mut self.mix.rev2_on, &mut self.mix.rev2)
            };
            let ch = if idx == 0 { &mut self.mix.ch1 } else { &mut self.mix.ch2 };
            let w3 = (ui.available_width() - 2.0 * gap) / 3.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                cell(ui, w3, |ui, head| if head { ui.label(RichText::new("Panoramique").color(TEXT)); } else {
                    spread(ui, 1, |ui, _| {
                        Knob { label: "Position", min: -1.0, max: 1.0, default: 0.0, step: 0.01, scale: Scale::Lin, bipolar: true,
                            format: &|v| if v.abs() < 0.005 { "Centre".into() } else if v < 0.0 { format!("G {:.0} %", -v * 100.0) } else { format!("D {:.0} %", v * 100.0) },
                            accent, enabled: true }.show(ui, pan);
                    });
                });
                cell(ui, w3, |ui, head| if head { toggle(ui, rev_on, "Reverb", REV); } else {
                    let on = *rev_on;
                    spread(ui, 1, |ui, _| {
                        Knob { label: "Envoi", min: -60.0, max: 6.0, default: -13.0, step: 0.5, scale: Scale::Lin, bipolar: false,
                            format: &|v| if v <= -60.0 { "-∞".into() } else { fr(format!("{v:+.1} dB")) }, accent: REV, enabled: on }.show(ui, rev);
                    });
                });
                cell(ui, w3, |ui, head| if head { toggle(ui, &mut ch.hpf_on, "Coupe-bas", accent); } else {
                    let on = ch.hpf_on;
                    spread(ui, 1, |ui, _| {
                        Knob { label: "Fréquence", min: 20.0, max: 500.0, default: 80.0, step: 1.0, scale: Scale::Log, bipolar: false,
                            format: &|v| fmt_hz(v), accent, enabled: on }.show(ui, &mut ch.hpf_freq);
                    });
                });
            });

            // Égaliseur : courbe puis quatre cases (2 × 2) sur toute la largeur
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Égaliseur").strong().color(TEXT));
                ui.add_space(6.0);
                toggle(ui, &mut ch.eq_on, "Actif", accent);
                ui.label(RichText::new("tire les points 1 à 4 et H, molette = largeur Q").color(DIM).size(12.0));
            });
            let graph_h = if idx == 0 { 150.0 + self.amp_h } else { 150.0 };
            let ch = if idx == 0 { &mut self.mix.ch1 } else { &mut self.mix.ch2 };
            eq_graph(ui, ch, accent, graph_h);
            ui.add_space(2.0);
            let names = ["Grave", "Bas-médium", "Haut-médium", "Aigu"];
            let w2 = (ui.available_width() - gap) / 2.0;
            for row in 0..2 {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for col in 0..2 {
                        let i = row * 2 + col;
                        let on = ch.eq_on;
                        let b = &mut ch.bands[i];
                        let mut kind = b.kind;
                        cell(ui, w2, |ui, head| if head {
                            ui.label(RichText::new(format!("{} · {}", i + 1, names[i])).color(TEXT).strong());
                            let choices = kind_choices(i);
                            if choices.len() > 1 {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    egui::ComboBox::from_id_salt(("type", idx, i)).width(118.0).selected_text(kind.label()).show_ui(ui, |ui| {
                                        for k in choices {
                                            ui.selectable_value(&mut kind, *k, k.label());
                                        }
                                    });
                                });
                            }
                        } else {
                            let is_bell = b.kind == state::Kind::Cloche;
                            spread(ui, 3, |ui, k| match k {
                                0 => { Knob { label: "Fréquence", min: 20.0, max: 20000.0, default: b.freq, step: 1.0, scale: Scale::Log, bipolar: false,
                                    format: &|v| fmt_hz(v), accent, enabled: on }.show(ui, &mut b.freq); }
                                1 => { Knob { label: "Gain", min: -18.0, max: 18.0, default: 0.0, step: 0.5, scale: Scale::Lin, bipolar: true,
                                    format: &|v| fr(format!("{v:+.1} dB")), accent, enabled: on }.show(ui, &mut b.gain); }
                                _ => { Knob { label: "Largeur Q", min: 0.1, max: 16.0, default: 1.0, step: 0.01, scale: Scale::Log, bipolar: false,
                                    format: &|v| fr(format!("{v:.2}")), accent, enabled: on && is_bell }.show(ui, &mut b.q); }
                            });
                        });
                        ch.bands[i].kind = kind;
                    }
                });
                ui.add_space(gap - ui.spacing().item_spacing.y);
            }

            // Compresseur : une case sur toute la largeur, courbe à droite
            let wf = ui.available_width();
            cell(ui, wf, |ui, head| if head { toggle(ui, &mut ch.comp_on, "Compresseur", accent); } else {
                let on = ch.comp_on;
                ui.horizontal(|ui| {
                    let graph = 80.0;
                    ui.allocate_ui_with_layout(vec2(ui.available_width() - graph - 8.0, 84.0), egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                        spread(ui, 5, |ui, k| match k {
                            0 => { Knob { label: "Seuil", min: -54.0, max: 0.0, default: -24.0, step: 1.0, scale: Scale::Lin, bipolar: false,
                                format: &|v| format!("{v:.0} dB"), accent, enabled: on }.show(ui, &mut ch.thresh); }
                            1 => { Knob { label: "Ratio", min: 1.0, max: RATIO_INF + 1.0, default: 4.0, step: 0.1, scale: Scale::Log, bipolar: false,
                                format: &|v| if v > RATIO_INF { "∞ : 1".into() } else { fr(format!("{v:.1} : 1")) }, accent, enabled: on }.show(ui, &mut ch.ratio); }
                            2 => { Knob { label: "Gain", min: 0.0, max: 18.0, default: 3.0, step: 0.1, scale: Scale::Lin, bipolar: false,
                                format: &|v| fr(format!("{v:+.1} dB")), accent, enabled: on }.show(ui, &mut ch.gain); }
                            3 => { Knob { label: "Attaque", min: 1.0, max: 100.0, default: 27.0, step: 1.0, scale: Scale::Log, bipolar: false,
                                format: &|v| format!("{v:.0} ms"), accent, enabled: on }.show(ui, &mut ch.attack); }
                            _ => { Knob { label: "Relâchement", min: 10.0, max: 2000.0, default: 123.0, step: 1.0, scale: Scale::Log, bipolar: false,
                                format: &|v| format!("{v:.0} ms"), accent, enabled: on }.show(ui, &mut ch.release); }
                        });
                    });
                    comp_graph(ui, ch, accent, graph);
                });
            });

            // Ampli (canal 2) : une case sur toute la largeur
            if idx == 1 {
                let top = ui.cursor().top();
                ui.add_space(gap - ui.spacing().item_spacing.y);
                let wf = ui.available_width();
                let amp_on = &mut self.mix.amp_on;
                let model = &mut self.mix.amp_model;
                cell(ui, wf, |ui, head| if head { toggle(ui, amp_on, "Simulateur d'ampli", accent); } else {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Modèle").color(DIM));
                        for (i, (n, _)) in AMP_MODELS.iter().enumerate() {
                            if ui.selectable_label(*model == i, *n).clicked() {
                                *model = i;
                            }
                        }
                        ui.label(RichText::new("réglages d'usine, potards à venir").color(DIM).size(12.0));
                    });
                });
                let h = ui.cursor().top() - top;
                if (h - self.amp_h).abs() > 0.5 {
                    self.amp_h = h;
                    ui.ctx().request_repaint();
                }
            }
        });
    }

    fn profile_header(&mut self, ui: &mut egui::Ui, idx: usize, accent: Color32, title: &str) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).size(17.0).strong().color(accent));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Enregistrer…").on_hover_text("Enregistrer ce réglage comme profil").clicked() {
                    self.save_dialog = Some((idx, String::new()));
                }
                let ch = if idx == 0 { &self.mix.ch1 } else { &self.mix.ch2 };
                let current = self.builtin.iter().chain(self.user.iter()).find(|p| &p.channel == ch).map(|p| p.name.clone());
                let mut pick: Option<Channel> = None;
                let mut delete: Option<String> = None;
                egui::ComboBox::from_id_salt(("profil", idx))
                    .width(250.0)
                    .selected_text(current.clone().unwrap_or_else(|| "Réglage personnalisé".into()))
                    .show_ui(ui, |ui| {
                        ui.label(RichText::new("Profils intégrés").color(DIM).size(12.0));
                        for p in &self.builtin {
                            if ui.selectable_label(current.as_deref() == Some(&p.name), &p.name).on_hover_text(&p.description).clicked() {
                                pick = Some(p.channel.clone());
                            }
                        }
                        if !self.user.is_empty() {
                            ui.separator();
                            ui.label(RichText::new("Mes profils").color(DIM).size(12.0));
                            for p in &self.user {
                                ui.horizontal(|ui| {
                                    if ui.selectable_label(current.as_deref() == Some(&p.name), &p.name).clicked() {
                                        pick = Some(p.channel.clone());
                                    }
                                    if ui.small_button("Supprimer").clicked() {
                                        delete = Some(p.name.clone());
                                    }
                                });
                            }
                        }
                    });
                ui.label(RichText::new("Profil").color(DIM));
                if let Some(c) = pick {
                    *(if idx == 0 { &mut self.mix.ch1 } else { &mut self.mix.ch2 }) = c;
                }
                if let Some(n) = delete {
                    self.user.retain(|p| p.name != n);
                    profiles::save_user(&self.user);
                }
            });
        });
        let ch_ref = if idx == 0 { &self.mix.ch1 } else { &self.mix.ch2 };
        let desc = self.builtin.iter().find(|p| &p.channel == ch_ref).map(|p| p.description.as_str()).unwrap_or("");
        // hauteur réservée même sans description, pour que les deux canaux restent alignés
        ui.add_sized(vec2(ui.available_width(), 30.0), egui::Label::new(RichText::new(desc).color(DIM).size(12.0)).wrap());
    }

    fn reverb_strip(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Reverb").size(17.0).strong().color(REV));
                ui.add_space(12.0);
                for (i, (n, _, hpf, lpf, time, damp)) in REVERBS.iter().enumerate() {
                    if ui.selectable_label(self.mix.reverb == i, *n).clicked() {
                        self.mix.reverb = i;
                        self.mix.reverb_hpf = *hpf;
                        self.mix.reverb_lpf = *lpf;
                        self.mix.reverb_time = *time;
                        self.mix.reverb_damp = *damp;
                    }
                }
                ui.add_space(20.0);
                Knob { label: "Niveau", min: -60.0, max: 6.0, default: 0.0, step: 0.5, scale: Scale::Lin, bipolar: false,
                    format: &|v| if v <= -60.0 { "-∞".into() } else { fr(format!("{v:+.1} dB")) }, accent: REV, enabled: true }
                    .show(ui, &mut self.mix.reverb_level);
                Knob { label: "Durée", min: 0.3, max: 30.0, default: 2.8, step: 0.1, scale: Scale::Log, bipolar: false,
                    format: &|v| fr(format!("{v:.1} s")), accent: REV, enabled: true }.show(ui, &mut self.mix.reverb_time);
                Knob { label: "Amortiss.", min: 1.0, max: 8.0, default: 7.0, step: 1.0, scale: Scale::Lin, bipolar: false,
                    format: &|v| format!("{v:.0}"), accent: REV, enabled: true }.show(ui, &mut self.mix.reverb_damp);
                Knob { label: "Coupe-bas", min: 20.0, max: 1000.0, default: 20.0, step: 1.0, scale: Scale::Log, bipolar: false,
                    format: &|v| fmt_hz(v), accent: REV, enabled: true }.show(ui, &mut self.mix.reverb_hpf);
                Knob { label: "Coupe-haut", min: 1000.0, max: 20000.0, default: 10000.0, step: 10.0, scale: Scale::Log, bipolar: false,
                    format: &|v| fmt_hz(v), accent: REV, enabled: true }.show(ui, &mut self.mix.reverb_lpf);
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Diffusion et pré-délai : ceux du type choisi (décodage en cours).").color(DIM).size(12.0));
                    ui.label(RichText::new("La reverb s'entend sur les canaux dont l'interrupteur Reverb est actif.").color(DIM).size(12.0));
                });
            });
        });
    }

    fn save_window(&mut self, ctx: &egui::Context) {
        let Some((idx, mut name)) = self.save_dialog.take() else { return };
        let mut open = true;
        let mut done = false;
        egui::Window::new("Enregistrer le profil").collapsible(false).resizable(false).open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Nom du profil :");
                let r = ui.text_edit_singleline(&mut name);
                r.request_focus();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ok = !name.trim().is_empty();
                    if ui.add_enabled(ok, egui::Button::new("Enregistrer")).clicked() || (ok && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                        let channel = if idx == 0 { self.mix.ch1.clone() } else { self.mix.ch2.clone() };
                        let n = name.trim().to_string();
                        self.user.retain(|p| p.name != n);
                        self.user.push(Profile { name: n, description: "Profil personnel".into(), channel });
                        profiles::save_user(&self.user);
                        done = true;
                    }
                    if ui.button("Annuler").clicked() {
                        done = true;
                    }
                });
            });
        if open && !done {
            self.save_dialog = Some((idx, name));
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, frames)) = self.screenshot.as_mut() else { return };
        *frames += 1;
        if *frames == 14 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
        let path = path.clone();
        ctx.input(|i| {
            for e in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = e {
                    let mut ppm = format!("P6 {} {} 255\n", image.size[0], image.size[1]).into_bytes();
                    for p in &image.pixels {
                        ppm.extend_from_slice(&[p.r(), p.g(), p.b()]);
                    }
                    let _ = std::fs::write(&path, ppm);
                    std::process::exit(0);
                }
            }
        });
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let st = self.midi.status();
        if st.generation != self.generation {
            self.generation = st.generation;
            self.last_sent.clear();
        }
        let top = egui::TopBottomPanel::top("barre").frame(egui::Frame::none().fill(BG).inner_margin(10.0)).show(ctx, |ui| {
            self.top_bar(ui, &st);
        });
        let bottom = egui::TopBottomPanel::bottom("reverb").frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(10.0, 8.0))).show(ctx, |ui| {
            self.reverb_strip(ui);
        });
        let content_h = egui::CentralPanel::default().frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(10.0, 0.0))).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.columns(2, |cols| {
                    self.channel(&mut cols[0], 0);
                    self.channel(&mut cols[1], 1);
                });
            }).content_size.y
        }).inner;
        // Au démarrage : la fenêtre prend exactement la hauteur du contenu (dans la limite de l'écran)
        if self.fit_frames < 4 {
            self.fit_frames += 1;
            if self.fit_frames == 4 {
                let want = top.response.rect.height() + bottom.response.rect.height() + content_h + 6.0;
                let max = ctx.input(|i| i.viewport().monitor_size).map(|m| m.y - 60.0).unwrap_or(want);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(1460.0, want.min(max))));
            } else {
                ctx.request_repaint();
            }
        }
        self.save_window(ctx);
        self.flush(ctx, st.connected);
        self.autosave();
        self.handle_screenshot(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Ok(s) = serde_json::to_string_pretty(&self.mix) {
            let _ = std::fs::write(state_path(), s);
        }
    }
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none().fill(CARD).rounding(10.0).inner_margin(12.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

/// Case encadrée de largeur fixe : un en-tête (hauteur fixe) puis le contenu.
fn cell(ui: &mut egui::Ui, width: f32, mut part: impl FnMut(&mut egui::Ui, bool)) {
    let margin = 8.0;
    let inner = width - 2.0 * margin;
    egui::Frame::none().fill(CELL).rounding(8.0).inner_margin(margin).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_width(inner);
            ui.allocate_ui_with_layout(vec2(inner, 26.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_width(inner);
                ui.set_height(26.0);
                part(ui, true);
            });
            part(ui, false);
        });
    });
}

/// Répartit `n` potards (64 px de large) régulièrement sur la largeur disponible.
fn spread(ui: &mut egui::Ui, n: usize, mut add: impl FnMut(&mut egui::Ui, usize)) {
    let kw = 64.0;
    let w = ui.available_width();
    let pad = ((w - n as f32 * kw) / n as f32 / 2.0).max(2.0);
    ui.horizontal(|ui| {
        ui.set_width(w);
        ui.spacing_mut().item_spacing.x = 0.0;
        for i in 0..n {
            ui.add_space(pad);
            add(ui, i);
            ui.add_space(pad);
        }
    });
}

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.extreme_bg_color = Color32::from_rgb(0x1A, 0x1D, 0x23);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x2C, 0x30, 0x38);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x38, 0x3D, 0x47);
    v.selection.bg_fill = Color32::from_rgb(0x4A, 0x40, 0x6A);
    v.override_text_color = Some(TEXT);
    ctx.set_visuals(v);
    let mut s = (*ctx.style()).clone();
    s.spacing.item_spacing = vec2(8.0, 6.0);
    s.spacing.button_padding = vec2(10.0, 5.0);
    ctx.set_style(s);
}

/// Écrit un message dans ag03studio-journal.txt, à côté de l'exécutable.
fn journal(msg: &str) {
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("ag03studio-journal.txt")))
        .unwrap_or_else(|| "ag03studio-journal.txt".into());
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{msg}");
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        journal(&format!("PLANTAGE : {info}\n{}", std::backtrace::Backtrace::force_capture()));
    }));
    journal("Démarrage");
    let run = |renderer| {
        let opts = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("AG03 MK2 Studio")
                .with_inner_size([1460.0, 1010.0])
                .with_min_inner_size([1100.0, 700.0])
                .with_icon(egui::IconData { rgba: include_bytes!("../../../assets/icone256.rgba").to_vec(), width: 256, height: 256 }),
            renderer,
            vsync: true,
            ..Default::default()
        };
        eframe::run_native("AG03 MK2 Studio", opts, Box::new(|cc| Ok(Box::new(App::new(cc)))))
    };
    // wgpu (DirectX 12 / Vulkan) évite le scintillement d'OpenGL sous Windows.
    // AG03_RENDU=opengl force l'ancien rendu ; si wgpu échoue, repli automatique sur OpenGL.
    let force_gl = std::env::var("AG03_RENDU").map(|v| v == "opengl").unwrap_or(false);
    if !force_gl {
        match run(eframe::Renderer::Wgpu) {
            Ok(()) => return,
            Err(e) => journal(&format!("wgpu indisponible ({e}), repli sur OpenGL")),
        }
    }
    if let Err(e) = run(eframe::Renderer::Glow) {
        journal(&format!("ERREUR au lancement de la fenêtre : {e}"));
    }
}
