//! EQ 4 bandes et passe-haut : calcul des coefficients comme AG Controller (vérifié à l'octet près
//! sur les 108 bandes (canaux 1 et 2) et 6 passe-haut des presets d'usine).
//!
//! - Biquads à 48 kHz, coefficients normalisés [b0, b1, b2, -a1, -a2].
//! - Cloche « boost/cut symétrique », shelfs Butterworth avec fréquence corrigée par √V.
//! - Gain arrondi au 0,5 dB.
//! - Bande 1 et passe-haut : 28 bits (Q26, 24 octets) ; bandes 2 à 4 : 16 bits (Q14, 12 octets).
//! - 28 bits : arrondi simple ; 16 bits : b1 recalculé pour un gain exact de 1 à 0 Hz.

use std::f64::consts::{PI, SQRT_2};

const FS: f64 = 48000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandType {
    Bell,
    LowShelf,
    HighShelf,
}

impl BandType {
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "bell" | "cloche" | "peak" => Some(Self::Bell),
            "lowshelf" | "low-shelf" => Some(Self::LowShelf),
            "highshelf" | "high-shelf" => Some(Self::HighShelf),
            _ => None,
        }
    }
}

fn bell(f0: f64, g: f64, q: f64) -> [f64; 5] {
    let w = 2.0 * PI * f0 / FS;
    let (c, a, v) = (w.cos(), w.sin() / (2.0 * q), 10f64.powf(g / 20.0));
    let (b0, b2, a0, a2) = if g >= 0.0 {
        (1.0 + a * v, 1.0 - a * v, 1.0 + a, 1.0 - a)
    } else {
        (1.0 + a, 1.0 - a, 1.0 + a / v, 1.0 - a / v)
    };
    [b0 / a0, -2.0 * c / a0, b2 / a0, 2.0 * c / a0, -a2 / a0]
}

fn shelf(high: bool, f0: f64, g: f64) -> [f64; 5] {
    let v = 10f64.powf(g.abs() / 20.0);
    let mut k = (PI * f0 / FS).tan();
    k = if high { k * v.sqrt() } else { k / v.sqrt() };
    let s = (2.0 * v).sqrt() * k;
    let num = if high {
        [v + s + k * k, 2.0 * (k * k - v), v - s + k * k]
    } else {
        [1.0 + s + v * k * k, 2.0 * (v * k * k - 1.0), 1.0 - s + v * k * k]
    };
    let den = [1.0 + SQRT_2 * k + k * k, 2.0 * (k * k - 1.0), 1.0 - SQRT_2 * k + k * k];
    let (num, den) = if g < 0.0 { (den, num) } else { (num, den) };
    [num[0] / den[0], num[1] / den[0], num[2] / den[0], -den[1] / den[0], -den[2] / den[0]]
}

fn highpass(f0: f64) -> [f64; 5] {
    let w = 2.0 * PI * f0 / FS;
    let (c, a) = (w.cos(), w.sin() / SQRT_2); // Q = 1/√2
    let a0 = 1.0 + a;
    [(1.0 + c) / 2.0 / a0, -(1.0 + c) / a0, (1.0 + c) / 2.0 / a0, 2.0 * c / a0, -(1.0 - a) / a0]
}

/// Place `width` bits de `v` à partir du bit `pos` dans un flux de 7 bits par octet (poids faible d'abord).
fn put_bits(out: &mut [u8], pos: usize, width: usize, v: i64) {
    for i in 0..width {
        if (v >> i) & 1 == 1 {
            let p = pos + i;
            out[p / 7] |= 1 << (p % 7);
        }
    }
}

fn round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

/// Encode 5 coefficients. 28 bits : arrondi simple. 16 bits : b1 recalculé pour un gain exact de 1 à 0 Hz.
fn encode(co: [f64; 5], wide: bool) -> Vec<u8> {
    let frac = if wide { 26 } else { 14 };
    let one = 1i64 << frac;
    let mut q: Vec<i64> = co.iter().map(|x| round(x * one as f64)).collect();
    if !wide {
        q[1] = (one - q[3] - q[4]) - q[0] - q[2];
    }
    let mut out = vec![0u8; if wide { 24 } else { 12 }];
    for (i, &x) in q.iter().enumerate() {
        if wide {
            put_bits(&mut out, 4 + 32 * i, 16, x >> 12);
            put_bits(&mut out, 20 + 32 * i, 12, x);
        } else {
            put_bits(&mut out, 4 + 16 * i, 16, x);
        }
    }
    out
}

fn body(addr: u8, wide: bool, value: Vec<u8>) -> Vec<u8> {
    let mut b = vec![0x00, 0x10, addr, 0x00, if wide { 0x24 } else { 0x10 }, 0x00];
    b.extend(value);
    b.push(0xF7);
    b
}

/// Adresses des bandes (Low, MidLow, MidHigh, High) et du passe-haut, par canal.
fn band_addr(ch: u8, band: usize) -> u8 {
    let ch1 = [0x4B, 0x55, 0x5A, 0x5F];
    let ch2 = [0x66, 0x70, 0x75, 0x7A];
    if ch == 1 { ch1[band] } else { ch2[band] }
}

pub fn band_index(name: &str) -> Option<usize> {
    ["low", "midlow", "midhigh", "high"].iter().position(|b| *b == name)
}

/// Coefficients normalisés [b0, b1, b2, -a1, -a2] d'une bande (gain arrondi au 0,5 dB comme le boîtier).
pub fn band_coefs(t: BandType, f0: f64, gain_db: f64, q: f64) -> [f64; 5] {
    let g = (gain_db * 2.0 + 0.5).floor() / 2.0;
    match t {
        BandType::Bell => bell(f0, g, q),
        BandType::LowShelf => shelf(false, f0, g),
        BandType::HighShelf => shelf(true, f0, g),
    }
}

/// Coefficients du passe-haut.
pub fn hpf_coefs(f0: f64) -> [f64; 5] {
    highpass(f0)
}

/// Réponse en dB d'un biquad à la fréquence `f`.
pub fn response_db(co: &[f64; 5], f: f64) -> f64 {
    let w = 2.0 * PI * f / FS;
    let (c1, s1, c2, s2) = (w.cos(), w.sin(), (2.0 * w).cos(), (2.0 * w).sin());
    let (nr, ni) = (co[0] + co[1] * c1 + co[2] * c2, -(co[1] * s1 + co[2] * s2));
    let (dr, di) = (1.0 - co[3] * c1 - co[4] * c2, co[3] * s1 + co[4] * s2);
    10.0 * ((nr * nr + ni * ni) / (dr * dr + di * di)).log10()
}

/// Corps du message pour une bande d'EQ. `band` : 0 = Low ... 3 = High.
pub fn band_body(ch: u8, band: usize, t: BandType, f0: f64, gain_db: f64, q: f64) -> Vec<u8> {
    let g = (gain_db * 2.0 + 0.5).floor() / 2.0;
    let co = match t {
        BandType::Bell => bell(f0, g, q),
        BandType::LowShelf => shelf(false, f0, g),
        BandType::HighShelf => shelf(true, f0, g),
    };
    let wide = band == 0;
    body(band_addr(ch, band), wide, encode(co, wide))
}

/// Corps du message pour la fréquence du passe-haut.
pub fn hpf_body(ch: u8, f0: f64) -> Vec<u8> {
    body(if ch == 1 { 0x15 } else { 0x21 }, true, encode(highpass(f0), true))
}

#[cfg(test)]
#[path = "eq_vectors.rs"]
mod vectors;

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|x| u8::from_str_radix(x, 16).unwrap()).collect()
    }

    #[test]
    fn all_factory_bands_match() {
        let mut bad = vec![];
        for &(ch, band, t, f, g, q, cap) in super::vectors::BANDS {
            let t = [BandType::Bell, BandType::LowShelf, BandType::HighShelf][t as usize];
            if band_body(ch, band, t, f, g, q) != h(cap) {
                bad.push((ch, band, t, f, g, q));
            }
        }
        assert!(bad.is_empty(), "{} écarts sur {} : {:?}", bad.len(), super::vectors::BANDS.len(), bad);
        assert_eq!(super::vectors::BANDS.len(), 108);
    }

    #[test]
    fn all_factory_highpass_match() {
        for &(ch, f, cap) in super::vectors::HPF {
            assert_eq!(hpf_body(ch, f), h(cap), "canal {ch}, HPF {f} Hz");
        }
    }

    #[test]
    fn channel2_uses_its_own_addresses() {
        assert_eq!(band_body(2, 0, BandType::Bell, 100.0, 0.0, 1.0)[2], 0x66);
        assert_eq!(band_body(2, 3, BandType::Bell, 100.0, 0.0, 1.0)[2], 0x7A);
        assert_eq!(hpf_body(2, 80.0)[2], 0x21);
    }

    #[test]
    fn gain_is_rounded_to_half_db() {
        assert_eq!(band_body(1, 3, BandType::Bell, 5600.0, 3.4, 1.0), band_body(1, 3, BandType::Bell, 5600.0, 3.5, 1.0));
    }
}
