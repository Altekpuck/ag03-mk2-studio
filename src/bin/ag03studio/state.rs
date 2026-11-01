//! Modèle des réglages et traduction en messages SysEx.

use ag03ctl::{comp, eq, presets, protocol::*, reverb};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Cloche,
    PlateauGrave,
    PlateauAigu,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Cloche => "Cloche",
            Kind::PlateauGrave => "Plateau grave",
            Kind::PlateauAigu => "Plateau aigu",
        }
    }
    fn to_eq(self) -> eq::BandType {
        match self {
            Kind::Cloche => eq::BandType::Bell,
            Kind::PlateauGrave => eq::BandType::LowShelf,
            Kind::PlateauAigu => eq::BandType::HighShelf,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub kind: Kind,
    pub freq: f64,
    pub gain: f64,
    pub q: f64,
}

impl Band {
    pub const fn new(kind: Kind, freq: f64, gain: f64, q: f64) -> Self {
        Band { kind, freq, gain, q }
    }
    pub fn coefs(&self) -> [f64; 5] {
        eq::band_coefs(self.kind.to_eq(), self.freq, self.gain, self.q)
    }
}

/// Traitement d'un canal : c'est ce que contient un profil.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub hpf_on: bool,
    pub hpf_freq: f64,
    pub eq_on: bool,
    pub bands: [Band; 4],
    pub comp_on: bool,
    pub thresh: f64,
    pub ratio: f64,
    pub gain: f64,
    pub attack: f64,
    pub release: f64,
}

/// RATIO au-delà de cette valeur = infini.
pub const RATIO_INF: f64 = 20.0;

impl Channel {
    /// Réglage « Default » d'AG Controller.
    pub fn ag_default() -> Self {
        Channel {
            hpf_on: false,
            hpf_freq: 125.0,
            eq_on: false,
            bands: [
                Band::new(Kind::Cloche, 106.0, -4.0, 4.0),
                Band::new(Kind::Cloche, 710.0, -1.0, 7.0),
                Band::new(Kind::Cloche, 2500.0, 1.5, 0.63),
                Band::new(Kind::Cloche, 4500.0, 0.0, 0.7),
            ],
            comp_on: false,
            thresh: -24.0,
            ratio: 4.0,
            gain: 3.1,
            attack: 27.0,
            release: 123.0,
        }
    }

    /// Réponse totale (dB) à la fréquence f, telle que le boîtier la produit.
    pub fn response_db(&self, f: f64) -> f64 {
        let mut db = 0.0;
        if self.hpf_on {
            db += eq::response_db(&eq::hpf_coefs(self.hpf_freq), f);
        }
        if self.eq_on {
            for b in &self.bands {
                db += eq::response_db(&b.coefs(), f);
            }
        }
        db
    }

    fn ratio_value(&self) -> f64 {
        if self.ratio > RATIO_INF { 1000.0 } else { self.ratio }
    }

    /// Sortie du compresseur (dB) pour une entrée (dB), avec le gain de compensation.
    pub fn comp_curve(&self, input: f64) -> f64 {
        if !self.comp_on {
            return input;
        }
        let r = self.ratio_value();
        let out = if input > self.thresh { self.thresh + (input - self.thresh) / r } else { input };
        out + self.gain
    }

    fn bodies(&self, ch: u8, out: &mut Vec<Vec<u8>>) {
        let n = if ch == 1 { "ch1" } else { "ch2" };
        out.push(toggle_body(&format!("{n}-hpf"), self.hpf_on).unwrap());
        out.push(eq::hpf_body(ch, self.hpf_freq));
        out.push(toggle_body(&format!("{n}-eq"), self.eq_on).unwrap());
        for (i, b) in self.bands.iter().enumerate() {
            out.push(eq::band_body(ch, i, b.kind.to_eq(), b.freq, b.gain, b.q));
        }
        out.push(toggle_body(&format!("{n}-comp"), self.comp_on).unwrap());
        out.push(comp::threshold_body(ch, self.thresh, self.ratio_value(), self.gain));
        out.push(comp::attack_body(ch, self.attack));
        out.push(comp::release_body(ch, self.release));
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mix {
    pub ch1: Channel,
    pub ch2: Channel,
    pub pan1: f64,
    pub pan2: f64,
    pub rev1_on: bool,
    pub rev2_on: bool,
    pub rev1: f64,
    pub rev2: f64,
    pub reverb: usize,
    pub reverb_level: f64,
    #[serde(default = "default_rev_hpf")]
    pub reverb_hpf: f64,
    #[serde(default = "default_rev_lpf")]
    pub reverb_lpf: f64,
    #[serde(default = "default_rev_time")]
    pub reverb_time: f64,
    #[serde(default = "default_rev_damp")]
    pub reverb_damp: f64,
    pub amp_on: bool,
    pub amp_model: usize,
    pub footswitch_mute: bool,
}

/// Modèles d'ampli : (nom affiché, preset d'AG Controller dont on reprend les réglages d'ampli).
pub const AMP_MODELS: [(&str, &str); 4] = [
    ("Clair", "E. Guitar / Clean"),
    ("Crunch", "E. Guitar / Crunch"),
    ("Distorsion", "E. Guitar / Distortion"),
    ("Par défaut", "Default"),
];

/// Reverbs : (nom affiché, preset d'AG Controller, HPF, LPF, TIME, HIGH DAMP de ce preset).
pub const REVERBS: [(&str, &str, f64, f64, f64, f64); 4] = [
    ("Par défaut", "Default", 20.0, 10000.0, 2.8, 7.0),
    ("Salle", "Hall", 20.0, 7000.0, 3.2, 7.0),
    ("Pièce", "Room", 90.0, 8000.0, 1.0, 7.0),
    ("Scène", "Stage", 20.0, 11000.0, 1.8, 7.0),
];

fn default_rev_time() -> f64 {
    2.8
}
fn default_rev_damp() -> f64 {
    7.0
}

fn default_rev_hpf() -> f64 {
    20.0
}
fn default_rev_lpf() -> f64 {
    10000.0
}

/// Valeur de potard en dB vers gain ; -60 dB = coupé.
pub fn fader_gain(db: f64) -> f64 {
    if db <= -60.0 { 0.0 } else { db_to_gain(db) }
}

fn body_of(msg: Vec<u8>) -> Vec<u8> {
    msg[9..].to_vec()
}

fn preset_bodies(bank: &str, name: &str, filter: impl Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
    presets::bank(bank)
        .and_then(|b| b.into_iter().find(|p| p.name == name))
        .map(|p| p.msgs.iter().filter_map(|m| presets::parse_hex(m).ok()).filter(|b| filter(b)).collect())
        .unwrap_or_default()
}

impl Default for Mix {
    fn default() -> Self {
        Mix {
            ch1: crate::profiles::builtin()[0].channel.clone(),
            ch2: Channel::ag_default(),
            pan1: 0.0,
            pan2: 0.0,
            rev1_on: false,
            rev2_on: false,
            rev1: -13.0,
            rev2: -13.0,
            reverb: 0,
            reverb_level: 0.0,
            reverb_hpf: 20.0,
            reverb_lpf: 10000.0,
            reverb_time: 2.8,
            reverb_damp: 7.0,
            amp_on: false,
            amp_model: 0,
            footswitch_mute: false,
        }
    }
}

impl Mix {
    /// Tous les messages qui décrivent l'état complet (corps sans en-tête).
    pub fn bodies(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let (l, r) = pan_gains(self.pan1);
        out.push(body_of(set_stereo(0, Param::Ch1Pan, l, r)));
        let (l, r) = pan_gains(self.pan2);
        out.push(body_of(set_stereo(0, Param::Ch2Pan, l, r)));
        out.push(body_of(set_gain(0, Param::Ch1RevSend, fader_gain(self.rev1))));
        out.push(body_of(set_gain(0, Param::Ch2RevSend, fader_gain(self.rev2))));
        out.push(body_of(set_switch(0, Switch::Ch1RevSend, self.rev1_on)));
        out.push(body_of(set_switch(0, Switch::Ch2RevSend, self.rev2_on)));
        self.ch1.bodies(1, &mut out);
        self.ch2.bodies(2, &mut out);
        // Ampli : réglages du modèle choisi (banque 01), puis l'interrupteur
        out.extend(preset_bodies("ch2", AMP_MODELS[self.amp_model].1, |b| b.len() > 4 && b[0] == 0x00 && b[3] == 0x01));
        out.push(amp_body(self.amp_on));
        // Reverb : blocs du type choisi, sauf les filtres d'entrée qui suivent les potards
        out.extend(preset_bodies("reverb", REVERBS[self.reverb].1, |b| !matches!(b[2], 0x46 | 0x49 | 0x79)));
        out.push(reverb::hpf_body(self.reverb_hpf));
        out.push(reverb::lpf_body(self.reverb_lpf));
        out.push(reverb::time_body(self.reverb_time, self.reverb_damp));
        out.push(body_of(set_gain(0, Param::ReverbLevel, fader_gain(self.reverb_level))));
        out.push(footswitch_body(self.footswitch_mute));
        out
    }
}

/// Clé d'un message : deux messages de même clé règlent le même paramètre.
pub fn key(body: &[u8]) -> Vec<u8> {
    if body[0] == 0x00 { body[..6].to_vec() } else { body[..3].to_vec() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_parameter_has_its_own_key() {
        let m = Mix::default();
        let bodies = m.bodies();
        let keys: HashSet<_> = bodies.iter().map(|b| key(b)).collect();
        assert_eq!(keys.len(), bodies.len(), "deux messages partagent une clé");
    }

    #[test]
    fn amp_and_reverb_presets_are_found() {
        for (_, n) in AMP_MODELS {
            assert!(!preset_bodies("ch2", n, |b| b[0] == 0 && b[3] == 1).is_empty(), "{n}");
        }
        for (_, n, _, _, _, _) in REVERBS {
            assert_eq!(preset_bodies("reverb", n, |_| true).len(), 4, "{n}");
        }
    }
}
