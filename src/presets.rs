//! Presets d'AG Controller, reconstitués à partir des captures (voir presets/build_presets.py).

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Preset {
    pub name: String,
    /// Corps des messages (commande ... F7), sans en-tête ni numéro de séquence.
    pub msgs: Vec<String>,
}

#[derive(Deserialize)]
struct Banks {
    ch1: Vec<Preset>,
    ch2: Vec<Preset>,
    reverb: Vec<Preset>,
}

const DATA: &str = include_str!("../presets/presets.json");

fn banks() -> Banks {
    serde_json::from_str(DATA).expect("presets.json invalide")
}

pub fn bank(name: &str) -> Option<Vec<Preset>> {
    let b = banks();
    match name {
        "ch1" => Some(b.ch1),
        "ch2" => Some(b.ch2),
        "reverb" => Some(b.reverb),
        _ => None,
    }
}

/// Cherche par numéro (à partir de 1) ou par nom (insensible à la casse, correspondance partielle).
pub fn find(bank: Vec<Preset>, query: &str) -> Result<Preset, String> {
    if let Ok(i) = query.parse::<usize>() {
        let n = bank.len();
        return bank.into_iter().nth(i.wrapping_sub(1)).ok_or(format!("numéro hors liste (1 à {n})"));
    }
    let q = query.to_lowercase();
    let mut hits: Vec<Preset> = bank.into_iter().filter(|p| p.name.to_lowercase().contains(&q)).collect();
    match hits.len() {
        1 => Ok(hits.remove(0)),
        0 => Err(format!("aucun preset ne contient « {query} »")),
        n => Err(format!("{n} presets correspondent à « {query} », précise ou utilise le numéro")),
    }
}

pub fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    s.split_whitespace()
        .map(|x| u8::from_str_radix(x, 16).map_err(|_| format!("octet invalide : {x}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_banks_load_and_parse() {
        for (b, count) in [("ch1", 21), ("ch2", 6), ("reverb", 4)] {
            let presets = bank(b).unwrap();
            assert_eq!(presets.len(), count, "{b}");
            for p in &presets {
                for m in &p.msgs {
                    let bytes = parse_hex(m).unwrap();
                    assert_eq!(*bytes.last().unwrap(), 0xF7, "{} : {m}", p.name);
                    assert!(bytes.iter().take(bytes.len() - 1).all(|&x| x < 0x80), "{}", p.name);
                }
            }
        }
    }

    #[test]
    fn find_by_name_and_number() {
        assert_eq!(find(bank("reverb").unwrap(), "hall").unwrap().name, "Hall");
        assert_eq!(find(bank("ch2").unwrap(), "1").unwrap().name, "E. Guitar / Crunch");
        assert!(find(bank("ch1").unwrap(), "voice").is_err()); // ambigu
    }
}
