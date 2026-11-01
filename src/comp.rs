//! Compresseur : calcul des blocs comme AG Controller (fonction 0x140027700 de AGController.exe,
//! vérifiée sur les presets d'usine des deux canaux).
//!
//! - Bloc « seuil » (8 octets) : trois mots Q15
//!   1. 10^(-X/20) avec X = tronc((1 - 1/R) × -THRESH)
//!   2. 10^(-Y/20) avec Y = tronc((1 - 1/R) × -THRESH - GAIN)
//!   3. (1/R) × 0,5, 1/R tronqué à 4 décimales (RATIO ∞ : 0)
//! - Bloc « attaque » (15 octets) : passe-bas bilinéaire du 1er ordre, fc = tronc(1000 / (2π × ATTACK)) Hz,
//!   coefficients b0, b1, a1 en 28 bits.
//! - Bloc « release » (3 octets) : arrondi(15,8121745 / (RELEASE en s)), 65535 au minimum (0,241 ms).
//!   AG Controller arrondit le RELEASE à ses propres crans : un preset « 109 ms » envoie en fait ~107 ms.

use std::f64::consts::PI;

const FS: f64 = 48000.0;

fn put_bits(out: &mut [u8], pos: usize, width: usize, v: i64) {
    for i in 0..width {
        if (v >> i) & 1 == 1 {
            let p = pos + i;
            out[p / 7] |= 1 << (p % 7);
        }
    }
}

fn trunc(x: f64) -> f64 {
    x.trunc()
}

fn q15(x: f64) -> i64 {
    let v = (x * 32768.0 + 0.5).floor() as i64;
    v.min(0x7FFF)
}

fn addrs(ch: u8) -> [u8; 3] {
    if ch == 1 { [0x2D, 0x32, 0x30] } else { [0x3C, 0x41, 0x3F] }
}

fn body(addr: u8, size_code: u8, value: Vec<u8>) -> Vec<u8> {
    let mut b = vec![0x00, 0x10, addr, 0x00, size_code, 0x00];
    b.extend(value);
    b.push(0xF7);
    b
}

/// THRESH (dB, ≤ 0), RATIO (≥ 1 ; 999 ou plus = ∞), GAIN (dB).
pub fn threshold_body(ch: u8, thresh: f64, ratio: f64, gain: f64) -> Vec<u8> {
    let inv = if ratio >= 999.0 { 0.0 } else { trunc(10000.0 / ratio) / 10000.0 };
    let gain = trunc(gain * 100.0) / 100.0;
    let x = trunc((1.0 - inv) * -thresh);
    let y = trunc((1.0 - inv) * -thresh - gain);
    let w = [
        q15(10f64.powf(-x / 20.0)),
        if y >= 0.0 { q15(10f64.powf(-y / 20.0)) } else { 0 },
        q15(inv * 0.5),
    ];
    let mut out = vec![0u8; 8];
    for (i, &v) in w.iter().enumerate() {
        put_bits(&mut out, 4 + 16 * i, 16, v);
    }
    body(addrs(ch)[0], 0x08, out)
}

/// ATTACK en ms.
pub fn attack_body(ch: u8, attack_ms: f64) -> Vec<u8> {
    // ATTACK 0 : pas de lissage (b0 = 1), comme AG Controller
    let co = if attack_ms <= 0.0 {
        [1.0, 0.0, 0.0]
    } else {
        let fc = trunc(1.0 / (attack_ms / 1000.0 * 2.0 * PI));
        let k = (PI * fc / FS).tan();
        [k / (1.0 + k), k / (1.0 + k), (1.0 - k) / (1.0 + k)]
    };
    let mut out = vec![0u8; 15];
    for (i, &c) in co.iter().enumerate() {
        let q = (c * 67108864.0 + 0.5).floor() as i64;
        put_bits(&mut out, 4 + 32 * i, 16, q >> 12);
        put_bits(&mut out, 20 + 32 * i, 12, q);
    }
    body(addrs(ch)[1], 0x14, out)
}

/// RELEASE en ms.
pub fn release_body(ch: u8, release_ms: f64) -> Vec<u8> {
    let r = trunc(release_ms * 1000.0) / 1000.0;
    let v = if r <= 0.241 {
        65535
    } else {
        let x = 15.812174479166666 / (r * 0.001);
        // arrondi au pair le plus proche, comme AG Controller
        let f = x.floor();
        let d = x - f;
        (if d > 0.5 || (d == 0.5 && f as i64 % 2 == 1) { f + 1.0 } else { f }) as i64
    };
    let mut out = vec![0u8; 3];
    put_bits(&mut out, 4, 16, v.min(65535));
    body(addrs(ch)[2], 0x00, out)
}

#[cfg(test)]
#[path = "comp_vectors.rs"]
mod vectors;

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|x| u8::from_str_radix(x, 16).unwrap()).collect()
    }

    #[test]
    fn factory_presets_match() {
        let mut rel_checked = 0;
        for &(ch, t, r, g, a, rel, thr_cap, att_cap, rel_cap) in super::vectors::COMP {
            assert_eq!(threshold_body(ch, t, r, g), h(thr_cap), "seuil canal {ch} {t}/{r}/{g}");
            assert_eq!(attack_body(ch, a), h(att_cap), "attaque canal {ch} {a} ms");
            if !rel_cap.is_empty() {
                assert_eq!(release_body(ch, rel), h(rel_cap), "release canal {ch} {rel} ms");
                rel_checked += 1;
            }
        }
        assert_eq!(super::vectors::COMP.len(), 11);
        assert!(rel_checked >= 6);
    }

    #[test]
    fn attack_27_and_30_give_same_block() {
        // fc entier : 5 Hz dans les deux cas
        assert_eq!(attack_body(1, 27.0), attack_body(1, 30.0));
    }
}
