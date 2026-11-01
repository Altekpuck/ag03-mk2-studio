//! Profils intégrés et profils enregistrés par l'utilisateur.

use crate::state::{Band, Channel, Kind::*};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub channel: Channel,
}

fn p(name: &str, description: &str, hpf: Option<f64>, bands: [Band; 4], comp: Option<[f64; 5]>) -> Profile {
    let c = comp.unwrap_or([-24.0, 4.0, 3.1, 27.0, 123.0]);
    Profile {
        name: name.into(),
        description: description.into(),
        channel: Channel {
            hpf_on: hpf.is_some(),
            hpf_freq: hpf.unwrap_or(80.0),
            eq_on: true,
            bands,
            comp_on: comp.is_some(),
            thresh: c[0],
            ratio: c[1],
            gain: c[2],
            attack: c[3],
            release: c[4],
        },
    }
}

/// Profils intégrés. Les réglages reprennent des pratiques éprouvées de traitement de la voix
/// (chaînes de type « coupe-bas, nettoyage des bas-médiums, présence, air, compresseur »).
pub fn builtin() -> Vec<Profile> {
    vec![
        p(
            "Kimura · Voix naturelle",
            "Micro électret de casque, près de la bouche. Coupe le souffle et l'effet de proximité, \
             éclaircit sans durcir. Compresseur doux.",
            Some(100.0),
            [
                Band::new(Cloche, 200.0, -2.0, 1.0),
                Band::new(Cloche, 450.0, -2.5, 1.4),
                Band::new(Cloche, 3500.0, 1.5, 1.0),
                Band::new(PlateauAigu, 10000.0, 2.0, 0.7),
            ],
            Some([-20.0, 3.0, 3.0, 10.0, 120.0]),
        ),
        p(
            "Kimura · Stream radio",
            "Voix pleine et devant, façon radio : compresseur rapide de style FET (type 1176), \
             présence et air marqués.",
            Some(90.0),
            [
                Band::new(Cloche, 150.0, 1.0, 0.8),
                Band::new(Cloche, 400.0, -3.0, 1.5),
                Band::new(Cloche, 2500.0, 2.0, 1.2),
                Band::new(PlateauAigu, 9000.0, 2.5, 0.7),
            ],
            Some([-24.0, 4.0, 6.0, 3.0, 80.0]),
        ),
        p(
            "Kimura · Tchat en jeu",
            "Intelligibilité maximale pour Discord ou le chat vocal : peu de graves, médiums mis en avant, \
             niveau très régulier.",
            Some(140.0),
            [
                Band::new(Cloche, 250.0, -3.0, 0.8),
                Band::new(Cloche, 1000.0, 0.0, 1.0),
                Band::new(Cloche, 3000.0, 2.5, 1.0),
                Band::new(PlateauAigu, 8000.0, 1.0, 0.7),
            ],
            Some([-26.0, 4.0, 6.0, 5.0, 100.0]),
        ),
        p(
            "Kimura · Anti-souffle et sifflantes",
            "Pour un micro de casque qui souffle ou siffle : coupe-bas plus haut et creux fixe vers 6,5 kHz \
             (sifflantes).",
            Some(120.0),
            [
                Band::new(Cloche, 300.0, -2.0, 1.0),
                Band::new(Cloche, 2000.0, 0.0, 1.0),
                Band::new(Cloche, 6500.0, -4.0, 4.0),
                Band::new(PlateauAigu, 12000.0, -1.0, 0.7),
            ],
            Some([-22.0, 3.0, 3.0, 15.0, 150.0]),
        ),
        p(
            "PodMic · Podcast chaleureux",
            "Micro dynamique : chaleur conservée, présence ajoutée. Compresseur lent de style optique \
             (type LA-2A).",
            Some(80.0),
            [
                Band::new(Cloche, 120.0, 1.5, 0.8),
                Band::new(Cloche, 250.0, -2.0, 1.0),
                Band::new(Cloche, 4500.0, 3.0, 0.8),
                Band::new(PlateauAigu, 10000.0, 3.0, 0.7),
            ],
            Some([-20.0, 3.0, 4.0, 20.0, 250.0]),
        ),
        p(
            "PodMic · Broadcast",
            "Micro dynamique, son radio dense : graves en plateau, bas-médiums nettoyés, compresseur rapide.",
            Some(70.0),
            [
                Band::new(PlateauGrave, 100.0, 2.0, 0.7),
                Band::new(Cloche, 350.0, -3.0, 1.4),
                Band::new(Cloche, 3000.0, 3.0, 1.0),
                Band::new(PlateauAigu, 12000.0, 2.0, 0.7),
            ],
            Some([-24.0, 4.0, 6.0, 2.0, 60.0]),
        ),
        p(
            "Guitare acoustique",
            "Repris du preset Acoustic d'AG Controller : coupe-bas, graves en plateau, un peu d'air.",
            Some(50.0),
            [
                Band::new(PlateauGrave, 75.0, -1.5, 0.7),
                Band::new(Cloche, 1000.0, 0.0, 4.5),
                Band::new(Cloche, 4000.0, 1.0, 1.8),
                Band::new(PlateauAigu, 12500.0, 3.0, 0.7),
            ],
            Some([-9.0, 2.5, 1.0, 17.0, 219.0]),
        ),
        Profile {
            name: "Neutre (tout à plat)".into(),
            description: "Aucun traitement : EQ à plat, compresseur et coupe-bas éteints.".into(),
            channel: Channel {
                hpf_on: false,
                hpf_freq: 80.0,
                eq_on: false,
                bands: [
                    Band::new(Cloche, 100.0, 0.0, 1.0),
                    Band::new(Cloche, 500.0, 0.0, 1.0),
                    Band::new(Cloche, 2000.0, 0.0, 1.0),
                    Band::new(Cloche, 8000.0, 0.0, 1.0),
                ],
                comp_on: false,
                thresh: -24.0,
                ratio: 4.0,
                gain: 0.0,
                attack: 27.0,
                release: 123.0,
            },
        },
        Profile {
            name: "AG Controller · Default".into(),
            description: "Le réglage par défaut d'AG Controller.".into(),
            channel: Channel::ag_default(),
        },
    ]
}

/// Dossier de configuration : %APPDATA%\AG03Studio sous Windows, ~/.config/ag03studio ailleurs.
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("AG03Studio"))
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(|p| PathBuf::from(p).join("ag03studio")))
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config").join("ag03studio")))
        .unwrap_or_else(|| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&base);
    base
}

pub fn load_user() -> Vec<Profile> {
    std::fs::read_to_string(config_dir().join("profils.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_user(list: &[Profile]) {
    if let Ok(s) = serde_json::to_string_pretty(list) {
        let _ = std::fs::write(config_dir().join("profils.json"), s);
    }
}
