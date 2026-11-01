//! Reverb : filtres d'entrée HPF et LPF (blocs 46 et 49 de la banque 04), vérifiés sur les 4 presets.
//!
//! Filtres bilinéaires du 1er ordre à 48 kHz, trois mots Q14 [b0, b1, -a1] au bit 4+16i.
//! LPF : b1 recalculé pour un gain exact de 1 à 0 Hz ; HPF : arrondi simple.
//!
//! Bloc 79 (42 octets) : quatre lignes de retard [x, g, g, c] + segment constant (641, 7).
//! - c : pôle du passe-bas de boucle, fc = 1920 × (HIGH DAMP + 3) Hz (exact sur HIGH DAMP 4 à 8).
//! - g : gain de boucle G = 2g / (16384 - c), loi RT60 sur une grille géométrique (q = 1,041296),
//!   les quatre lignes décalées de 0, 4, 8 et 11 crans. AG Controller arrondit TIME sur cette grille ;
//!   ici la loi est continue (écart inférieur à 1,5 % sur le gain, inaudible).
//! - x = 16384 × √((1 - G²) / 24) (normalisation d'énergie).
//! DIFFUSION et INI DELAY (bloc 57) restent ceux du type de reverb choisi.

use std::f64::consts::PI;

fn put_bits(out: &mut [u8], pos: usize, width: usize, v: i64) {
    for i in 0..width {
        if (v >> i) & 1 == 1 {
            let p = pos + i;
            out[p / 7] |= 1 << (p % 7);
        }
    }
}

fn round(x: f64) -> i64 {
    (x * 16384.0 + 0.5).floor() as i64
}

fn body(addr: u8, w: [i64; 3]) -> Vec<u8> {
    let mut v = vec![0u8; 8];
    for (i, &x) in w.iter().enumerate() {
        put_bits(&mut v, 4 + 16 * i, 16, x);
    }
    let mut b = vec![0x00, 0x10, addr, 0x04, 0x08, 0x00];
    b.extend(v);
    b.push(0xF7);
    b
}

/// Passe-haut d'entrée de la reverb.
pub fn hpf_body(freq: f64) -> Vec<u8> {
    let k = (PI * freq / 48000.0).tan();
    let b0 = round(1.0 / (1.0 + k));
    body(0x46, [b0, -b0, round((1.0 - k) / (1.0 + k))])
}

/// Passe-bas d'entrée de la reverb.
pub fn lpf_body(freq: f64) -> Vec<u8> {
    let k = (PI * freq / 48000.0).tan();
    let b0 = round(k / (1.0 + k));
    let a1 = round((1.0 - k) / (1.0 + k));
    body(0x49, [b0, 16384 - a1 - b0, a1])
}

const GRID_Q: f64 = 1.041296;
const GRID_U0: f64 = 3.95157; // -1/ln(G) de la 1re ligne à TIME = 2,8 s
const LINE_OFFSETS: [f64; 4] = [0.0, 4.0, 8.0, 11.0];

/// Pôle du passe-bas de boucle (Q14) pour HIGH DAMP.
pub fn high_damp_pole(high_damp: f64) -> i64 {
    let fc = (1920.0 * (high_damp + 3.0)).min(23900.0);
    let k = (PI * fc / 48000.0).tan();
    round((1.0 - k) / (1.0 + k))
}

/// Gain de boucle G de chaque ligne pour TIME (s).
pub fn loop_gains(time_s: f64) -> [f64; 4] {
    let n = (time_s.max(0.1) / 2.8).ln() / GRID_Q.ln() - 0.25;
    LINE_OFFSETS.map(|off| (-1.0 / (GRID_U0 * GRID_Q.powf(n + off))).exp())
}

/// Bloc 79 : TIME (0,3 à 30 s) et HIGH DAMP (1 à 8).
pub fn time_body(time_s: f64, high_damp: f64) -> Vec<u8> {
    let c = high_damp_pole(high_damp);
    let mut w: Vec<i64> = Vec::with_capacity(18);
    let gains = loop_gains(time_s);
    for (i, gl) in gains.iter().enumerate() {
        let g = (gl * (16384 - c) as f64 / 2.0 + 0.5).floor() as i64;
        let x = (16384.0 * ((1.0 - gl * gl) / 24.0).sqrt() + 0.5).floor() as i64;
        w.extend([x, g, g, c]);
        if i == 1 {
            w.extend([641, 7]); // en-tête du 2e segment (adresse, longueur)
        }
    }
    let mut v = vec![0u8; 42];
    for (i, &x) in w.iter().enumerate() {
        put_bits(&mut v, 4 + 16 * i, 16, x);
    }
    let mut b = vec![0x00, 0x10, 0x79, 0x04, 0x1C, 0x00];
    b.extend(v);
    b.push(0xF7);
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    fn block(preset: &str, addr: &str) -> Vec<u8> {
        let p = presets::bank("reverb").unwrap().into_iter().find(|p| p.name == preset).unwrap();
        presets::parse_hex(p.msgs.iter().find(|m| m.starts_with(&format!("00 10 {addr}"))).unwrap()).unwrap()
    }

    fn words(body: &[u8]) -> Vec<i64> {
        let v = &body[6..body.len() - 1];
        (0..18)
            .map(|k| {
                let mut x = 0i64;
                for j in 0..16 {
                    let p = 4 + 16 * k + j;
                    x |= (((v[p / 7] >> (p % 7)) & 1) as i64) << j;
                }
                if x & 0x8000 != 0 { x - 0x10000 } else { x }
            })
            .collect()
    }

    #[test]
    fn high_damp_pole_is_exact() {
        for (hd, c) in [(8.0, -11135), (7.0, -8348), (6.0, -5899), (5.0, -3662), (4.0, -1549)] {
            assert_eq!(high_damp_pole(hd), c, "HIGH DAMP {hd}");
        }
    }

    #[test]
    fn time_block_is_close_to_the_presets() {
        // (preset, TIME, HIGH DAMP)
        for (name, t, hd) in [("Default", 2.8, 7.0), ("Hall", 3.2, 7.0), ("Room", 1.0, 7.0), ("Stage", 1.8, 7.0)] {
            let cap = words(&block(name, "79"));
            let ours = words(&time_body(t, hd));
            assert_eq!(cap[8..10], ours[8..10]);
            for k in [0, 4, 10, 14] {
                assert_eq!(cap[k + 3], ours[k + 3], "{name} pôle");
                let gc = 2.0 * cap[k + 1] as f64 / (16384 - cap[k + 3]) as f64;
                let go = 2.0 * ours[k + 1] as f64 / (16384 - ours[k + 3]) as f64;
                assert!((gc - go).abs() / gc < 0.015, "{name} ligne {k} : {gc} / {go}");
                assert!((cap[k] - ours[k]).abs() <= 40, "{name} x ligne {k} : {} / {}", cap[k], ours[k]);
            }
        }
    }

    #[test]
    fn filters_match_the_four_presets() {
        // Fréquences réellement envoyées par AG Controller (il arrondit les valeurs de ses presets)
        for (name, hpf, lpf) in [("Default", 20.0, 10000.0), ("Hall", 20.0, 7000.0), ("Room", 90.0, 8000.0), ("Stage", 20.0, 11000.0)] {
            assert_eq!(hpf_body(hpf), block(name, "46"), "{name} HPF");
            assert_eq!(lpf_body(lpf), block(name, "49"), "{name} LPF");
        }
    }
}
