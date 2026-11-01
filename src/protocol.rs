//! Protocole SysEx de l'AG03 MK2 (déduit par capture d'AG Controller, voir PROTOCOLE.md).
// Les décodeurs servent aux tests et serviront à lire l'état du boîtier.
#![allow(dead_code)]

/// En-tête commun : Yamaha (43), envoi (10), modèle (3E 1C 78), octet constant (00).
const HEADER_SEND: [u8; 7] = [0xF0, 0x43, 0x10, 0x3E, 0x1C, 0x78, 0x00];

/// Gain unité en virgule fixe Q14.
const Q14_ONE: f64 = 16384.0;

/// Paramètres connus (mode simple).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Param {
    Ch1Pan,
    Ch2Pan,
    Ch1RevSend,
    Ch2RevSend,
    ReverbLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Mono,
    Stereo,
}

impl Param {
    pub const ALL: [Param; 5] = [
        Param::Ch1Pan,
        Param::Ch2Pan,
        Param::Ch1RevSend,
        Param::Ch2RevSend,
        Param::ReverbLevel,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Param::Ch1Pan => "ch1-pan",
            Param::Ch2Pan => "ch2-pan",
            Param::Ch1RevSend => "ch1-rev-send",
            Param::Ch2RevSend => "ch2-rev-send",
            Param::ReverbLevel => "reverb-level",
        }
    }

    pub fn from_name(s: &str) -> Option<Param> {
        Param::ALL.into_iter().find(|p| p.name() == s)
    }

    fn address(self) -> [u8; 5] {
        match self {
            Param::Ch1Pan => [0x10, 0x0E, 0x05, 0x04, 0x00],
            Param::Ch2Pan => [0x10, 0x11, 0x05, 0x04, 0x00],
            Param::Ch1RevSend => [0x10, 0x09, 0x04, 0x00, 0x00],
            Param::Ch2RevSend => [0x10, 0x0A, 0x04, 0x00, 0x00],
            Param::ReverbLevel => [0x10, 0x0C, 0x05, 0x04, 0x00],
        }
    }

    fn kind(self) -> Kind {
        match self {
            Param::Ch1RevSend | Param::Ch2RevSend => Kind::Mono,
            _ => Kind::Stereo,
        }
    }

    pub fn is_pan(self) -> bool {
        matches!(self, Param::Ch1Pan | Param::Ch2Pan)
    }
}

/// Interrupteurs connus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
    Ch1RevSend,
    Ch2RevSend,
}

impl Switch {
    fn index(self) -> u8 {
        match self {
            Switch::Ch1RevSend => 0x00,
            Switch::Ch2RevSend => 0x01,
        }
    }
}

fn q14(g: f64) -> u64 {
    (g.clamp(0.0, 3.99) * Q14_ONE).round() as u64
}

fn split7(mut n: u64, count: usize) -> Vec<u8> {
    (0..count)
        .map(|_| {
            let b = (n & 0x7F) as u8;
            n >>= 7;
            b
        })
        .collect()
}

fn join7(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .enumerate()
        .fold(0u64, |n, (i, &b)| n | ((b as u64 & 0x7F) << (7 * i)))
}

/// Valeur mono (3 octets).
pub fn encode_mono(gain: f64) -> Vec<u8> {
    split7(q14(gain) << 4, 3)
}

/// Valeur stéréo (6 octets) : gains gauche et droite.
pub fn encode_stereo(left: f64, right: f64) -> Vec<u8> {
    split7((q14(left) << 4) | (q14(right) << 20), 6)
}

pub fn decode_mono(bytes: &[u8]) -> f64 {
    (join7(bytes) >> 4) as f64 / Q14_ONE
}

pub fn decode_stereo(bytes: &[u8]) -> (f64, f64) {
    let n = join7(bytes);
    (
        ((n >> 4) & 0xFFFF) as f64 / Q14_ONE,
        ((n >> 20) & 0xFFFF) as f64 / Q14_ONE,
    )
}

pub fn db_to_gain(db: f64) -> f64 {
    if db <= -120.0 {
        0.0
    } else {
        10f64.powf(db / 20.0)
    }
}

/// Loi de panoramique à puissance constante observée : centre = 1,0 / 1,0, extrêmes = √2 / 0.
/// `pos` de -1,0 (gauche) à +1,0 (droite).
pub fn pan_gains(pos: f64) -> (f64, f64) {
    let angle = (pos.clamp(-1.0, 1.0) + 1.0) * std::f64::consts::FRAC_PI_4;
    let s = std::f64::consts::SQRT_2;
    (s * angle.cos(), s * angle.sin())
}

fn push_seq(msg: &mut Vec<u8>, seq: u16) {
    msg.push((seq & 0x7F) as u8);
    msg.push(((seq >> 7) & 0x7F) as u8);
}

/// Message « régler un paramètre » avec un gain identique des deux côtés (ou mono).
pub fn set_gain(seq: u16, param: Param, gain: f64) -> Vec<u8> {
    let value = match param.kind() {
        Kind::Mono => encode_mono(gain),
        Kind::Stereo => encode_stereo(gain, gain),
    };
    set_raw(seq, param, &value)
}

/// Message « régler un paramètre » stéréo avec gains G/D explicites (PAN).
pub fn set_stereo(seq: u16, param: Param, left: f64, right: f64) -> Vec<u8> {
    set_raw(seq, param, &encode_stereo(left, right))
}

fn set_raw(seq: u16, param: Param, value: &[u8]) -> Vec<u8> {
    let mut m = HEADER_SEND.to_vec();
    push_seq(&mut m, seq);
    m.push(0x00); // cmd : paramètre
    m.extend_from_slice(&param.address());
    m.extend_from_slice(value);
    m.push(0xF7);
    m
}

/// Message « interrupteur ».
pub fn set_switch(seq: u16, sw: Switch, on: bool) -> Vec<u8> {
    let mut m = HEADER_SEND.to_vec();
    push_seq(&mut m, seq);
    m.extend_from_slice(&[0x30, 0x10, sw.index(), if on { 0x02 } else { 0x00 }, 0x00, 0xF7]);
    m
}

/// Construit un message complet à partir d'un corps capturé (commande ... F7).
pub fn from_body(seq: u16, body: &[u8]) -> Vec<u8> {
    let mut m = HEADER_SEND.to_vec();
    push_seq(&mut m, seq);
    m.extend_from_slice(body);
    m
}

/// Interrupteurs « on/off » du mode détaillé (valeur 6 octets : 00 00 10 00 00 00 = off, 00 00 00 00 40 00 = on).
pub fn toggle_body(name: &str, on: bool) -> Option<Vec<u8>> {
    let addr: [u8; 5] = match name {
        "ch1-comp" => [0x10, 0x39, 0x00, 0x04, 0x00],
        "ch1-eq" => [0x10, 0x64, 0x00, 0x04, 0x00],
        "ch1-hpf" => [0x10, 0x1F, 0x00, 0x04, 0x00],
        // Déduit (même écart que le canal 1 : coefficients 0x21 + 0x0A), pas encore vérifié
        "ch2-hpf" => [0x10, 0x2B, 0x00, 0x04, 0x00],
        "ch2-comp" => [0x10, 0x48, 0x00, 0x04, 0x00],
        "ch2-eq" => [0x10, 0x7F, 0x00, 0x04, 0x00],
        _ => return None,
    };
    let mut b = vec![0x00];
    b.extend_from_slice(&addr);
    b.extend_from_slice(if on { &[0x00, 0x00, 0x00, 0x00, 0x40, 0x00] } else { &[0x00, 0x00, 0x10, 0x00, 0x00, 0x00] });
    b.push(0xF7);
    Some(b)
}

/// AMP SIM du canal 2 : on/off.
pub fn amp_body(on: bool) -> Vec<u8> {
    vec![0x30, 0x10, 0x03, if on { 0x02 } else { 0x00 }, 0x00, 0xF7]
}

/// AMP SIM du canal 2 : HIGH ou LOW (valeurs capturées telles quelles).
pub fn amp_type_body(high: bool) -> Vec<u8> {
    let v: [u8; 8] = if high {
        [0x70, 0x7F, 0x1F, 0x00, 0x00, 0x66, 0x01, 0x00]
    } else {
        [0x00, 0x00, 0x00, 0x2A, 0x02, 0x00, 0x00, 0x00]
    };
    let mut b = vec![0x00, 0x10, 0x09, 0x01, 0x08, 0x00];
    b.extend_from_slice(&v);
    b.push(0xF7);
    b
}

/// Pédale : REVERB (false) ou CH1 MUTE ON/OFF (true).
pub fn footswitch_body(ch1_mute: bool) -> Vec<u8> {
    vec![0x50, 0x10, 0x10, if ch1_mute { 0x02 } else { 0x00 }, 0x00, 0xF7]
}

/// Accusé de réception : renvoie (seq, cmd, état) si le message en est un.
pub fn parse_ack(msg: &[u8]) -> Option<(u16, u8, u8)> {
    if msg.len() >= 13 && msg[..7] == [0xF0, 0x43, 0x30, 0x3E, 0x1C, 0x78, 0x00] {
        let seq = msg[7] as u16 | ((msg[8] as u16) << 7);
        Some((seq, msg[9], msg[10]))
    } else {
        None
    }
}

/// Vrai pour les messages de vumètres (50 par seconde).
pub fn is_meter(msg: &[u8]) -> bool {
    msg.len() > 12 && msg[2] == 0x10 && msg[9] == 0x41 && msg[10] == 0x60
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|x| u8::from_str_radix(x, 16).unwrap()).collect()
    }

    // Messages capturés tels quels dans AG Controller.
    #[test]
    fn rev_send_minus_12db_matches_capture() {
        let captured = h("F0 43 10 3E 1C 78 00 03 01 00 10 09 04 00 00 30 02 04 F7");
        let g = decode_mono(&captured[15..18]);
        assert!((20.0 * g.log10() + 12.0).abs() < 0.05);
        assert_eq!(set_raw(0x83, Param::Ch1RevSend, &encode_mono(g)), captured);
    }

    #[test]
    fn reverb_level_max_matches_capture() {
        let captured = h("F0 43 10 3E 1C 78 00 72 00 00 10 0C 05 04 00 00 00 10 00 40 00 F7");
        assert_eq!(set_gain(0x72, Param::ReverbLevel, 1.0), captured);
    }

    #[test]
    fn pan_extremes_match_capture() {
        let (l, r) = decode_stereo(&h("20 50 16 00 00"));
        assert!((l - 2f64.sqrt()).abs() < 1e-3 && r == 0.0);
        let (l, r) = decode_stereo(&h("00 00 00 41 5A"));
        assert!(l == 0.0 && (r - 2f64.sqrt()).abs() < 1e-3);
        let (l, r) = pan_gains(-1.0);
        assert_eq!(encode_stereo(l, r), h("20 50 16 00 00 00"));
    }

    #[test]
    fn stereo_roundtrip_is_exact() {
        for v in ["30 32 10 31 3E 00", "60 65 01 17 07 00", "10 27 09 40 52 00"] {
            let (l, r) = decode_stereo(&h(v));
            assert_eq!(encode_stereo(l, r), h(v), "{v}");
        }
    }

    #[test]
    fn switch_matches_capture() {
        assert_eq!(
            set_switch(0x90, Switch::Ch1RevSend, false),
            h("F0 43 10 3E 1C 78 00 10 01 30 10 00 00 00 F7")
        );
        assert_eq!(
            set_switch(0xA3, Switch::Ch2RevSend, true),
            h("F0 43 10 3E 1C 78 00 23 01 30 10 01 02 00 F7")
        );
    }

    #[test]
    fn sequence_wraps_on_7_bits() {
        let m = set_gain(0x80, Param::ReverbLevel, 1.0);
        assert_eq!(&m[7..9], &[0x00, 0x01]);
    }

    #[test]
    fn pan_center_matches_capture() {
        // Capture brute : PAN canal 1 au centre (séquence 0x75)
        let captured = h("F0 43 10 3E 1C 78 00 75 00 00 10 0E 05 04 00 30 32 10 31 3E 00 F7");
        let (l, r) = decode_stereo(&captured[15..21]);
        assert_eq!(set_stereo(0x75, Param::Ch1Pan, l, r), captured);
    }

    #[test]
    fn new_bodies_match_captures() {
        assert_eq!(toggle_body("ch1-comp", false).unwrap(), h("00 10 39 00 04 00 00 00 10 00 00 00 F7"));
        assert_eq!(toggle_body("ch2-comp", true).unwrap(), h("00 10 48 00 04 00 00 00 00 00 40 00 F7"));
        assert_eq!(amp_body(true), h("30 10 03 02 00 F7"));
        assert_eq!(amp_type_body(false), h("00 10 09 01 08 00 00 00 00 2A 02 00 00 00 F7"));
        assert_eq!(footswitch_body(true), h("50 10 10 02 00 F7"));
        assert_eq!(from_body(0xA3, &h("30 10 01 02 00 F7")), set_switch(0xA3, Switch::Ch2RevSend, true));
    }

    #[test]
    fn ack_and_meter_are_recognised() {
        assert_eq!(
            parse_ack(&h("F0 43 30 3E 1C 78 00 27 01 30 50 00 00 F7")),
            Some((0xA7, 0x30, 0x50))
        );
        assert!(is_meter(&h(
            "F0 43 10 3E 1C 78 00 24 54 41 60 0A 02 4C 00 00 00 40 14 14 5A 65 70 00 F7"
        )));
    }
}
