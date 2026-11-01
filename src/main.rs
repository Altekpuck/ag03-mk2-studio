//! ag03ctl : contrôle de la Yamaha AG03 MK2 en MIDI SysEx, sans AG Controller.
//!
//! Par défaut, n'envoie RIEN : affiche seulement les messages qui seraient envoyés.
//! Ajouter `--send` pour les envoyer réellement au boîtier.

use ag03ctl::{comp, eq, presets, protocol};

use midir::{MidiInput, MidiOutput};
use protocol::*;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const USAGE: &str = "\
Usage :
  ag03ctl list                                 liste les ports MIDI
  ag03ctl monitor [secondes]                   affiche les messages du boîtier (hors vumètres)

  Réglages simples
  ag03ctl set <param> <dB>                     ex. set ch1-rev-send -12
  ag03ctl pan <ch1|ch2> <-1.0..1.0>            -1 = gauche, 0 = centre, 1 = droite
  ag03ctl switch <ch1|ch2> <on|off>            interrupteur REV SEND (= bouton REVERB de la table pour ch1)

  Mode détaillé
  ag03ctl toggle <nom> <on|off>                ch1-comp ch1-eq ch1-hpf ch2-comp ch2-eq ch2-hpf
  ag03ctl amp <on|off>                         AMP SIM du canal 2
  ag03ctl amp-type <high|low>                  type d'ampli du canal 2
  ag03ctl footswitch <reverb|ch1-mute>         fonction de la pédale

  Réglage libre (calcul identique à AG Controller)
  ag03ctl eq <ch1|ch2> <bande> <type> <Hz> <dB> [Q]
                                               bande : low midlow midhigh high
                                               type  : bell, lowshelf (bande low), highshelf (bande high)
                                               ex. eq ch1 high highshelf 8000 3
  ag03ctl hpf <ch1|ch2> <Hz>                   fréquence du filtre passe-haut (allumer : toggle ch1-hpf on)
  ag03ctl comp <ch1|ch2> <THRESH> <RATIO> <GAIN> <ATTACK> <RELEASE>
                                               compresseur, ex. comp ch1 -18 3 2 35 218
                                               (RATIO inf = infini ; ATTACK et RELEASE en ms ;
                                               allumer : toggle ch1-comp on)

  Presets (repris d'AG Controller)
  ag03ctl presets <ch1|ch2|reverb>             liste les presets
  ag03ctl preset <ch1|ch2|reverb> <n°|nom>     applique un preset complet

  Expert
  ag03ctl raw \"<octets hex>\"                   envoie un corps brut (commande ... F7)

Options :
  --send            envoie vraiment (sinon simple affichage)
  --port <texte>    nom (ou partie du nom) du port MIDI, défaut « AG03 »
  --seq <n>         premier numéro de séquence (défaut 1, le boîtier accepte n'importe lequel)

Paramètres de « set » : ch1-rev-send ch2-rev-send reverb-level (et ch1-pan, ch2-pan via « pan »)";

struct Opts {
    send: bool,
    port: String,
    seq: u16,
    args: Vec<String>,
}

fn parse_opts() -> Opts {
    let mut o = Opts { send: false, port: "AG03".into(), seq: 1, args: vec![] };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--send" => o.send = true,
            "--port" => o.port = it.next().unwrap_or_default(),
            "--seq" => o.seq = it.next().and_then(|s| s.parse().ok()).unwrap_or(1),
            _ => o.args.push(a),
        }
    }
    o
}

fn fail(msg: &str) -> ! {
    eprintln!("{msg}\n\n{USAGE}");
    std::process::exit(2);
}

fn on_off(s: &str) -> bool {
    match s {
        "on" => true,
        "off" => false,
        _ => fail("état attendu : on ou off"),
    }
}

fn list() -> Result<(), Box<dyn std::error::Error>> {
    let out = MidiOutput::new("ag03ctl")?;
    let inp = MidiInput::new("ag03ctl")?;
    println!("Sorties MIDI :");
    for p in out.ports() {
        println!("  {}", out.port_name(&p)?);
    }
    println!("Entrées MIDI :");
    for p in inp.ports() {
        println!("  {}", inp.port_name(&p)?);
    }
    Ok(())
}

/// Envoie les messages dans l'ordre et attend l'accusé de réception de chacun.
fn send_all(port_filter: &str, msgs: &[Vec<u8>]) -> Result<(), Box<dyn std::error::Error>> {
    let mut inp = MidiInput::new("ag03ctl-in")?;
    inp.ignore(midir::Ignore::None); // ne pas filtrer les SysEx
    let in_port = inp
        .ports()
        .into_iter()
        .find(|p| inp.port_name(p).map(|n| n.contains(port_filter)).unwrap_or(false))
        .ok_or("port MIDI d'entrée introuvable")?;
    let (tx, rx) = mpsc::channel();
    let _conn_in = inp.connect(&in_port, "ag03ctl-in", move |_, m, _| {
        if let Some(ack) = parse_ack(m) {
            let _ = tx.send(ack);
        }
    }, ())?;

    let out = MidiOutput::new("ag03ctl-out")?;
    let out_port = out
        .ports()
        .into_iter()
        .find(|p| out.port_name(p).map(|n| n.contains(port_filter)).unwrap_or(false))
        .ok_or("port MIDI de sortie introuvable")?;
    let mut conn = out.connect(&out_port, "ag03ctl-out")?;

    let mut acked = 0;
    for msg in msgs {
        let seq = msg[7] as u16 | ((msg[8] as u16) << 7);
        conn.send(msg)?;
        let deadline = Instant::now() + Duration::from_millis(500);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(left) {
                Ok((s, _, _)) if s == seq => {
                    acked += 1;
                    break;
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }
    }
    println!("Envoyé : {} message(s), {} accusé(s) de réception.", msgs.len(), acked);
    if acked < msgs.len() {
        println!("Certains messages n'ont pas été acquittés (le boîtier les a peut-être ignorés).");
    }
    Ok(())
}

fn monitor(port_filter: &str, secs: u64) -> Result<(), Box<dyn std::error::Error>> {
    let mut inp = MidiInput::new("ag03ctl-mon")?;
    inp.ignore(midir::Ignore::None);
    let port = inp
        .ports()
        .into_iter()
        .find(|p| inp.port_name(p).map(|n| n.contains(port_filter)).unwrap_or(false))
        .ok_or("port MIDI d'entrée introuvable")?;
    let _c = inp.connect(&port, "ag03ctl-mon", |ts, m, _| {
        if !is_meter(m) {
            println!("[{ts}] {}", hex(m));
        }
    }, ())?;
    println!("Écoute pendant {secs} s (vumètres masqués)...");
    std::thread::sleep(Duration::from_secs(secs));
    Ok(())
}

fn channel(s: &str) -> u8 {
    match s {
        "ch1" => 1,
        "ch2" => 2,
        _ => fail("canal attendu : ch1 ou ch2"),
    }
}

fn num(s: &str, min: f64, max: f64, what: &str) -> f64 {
    match s.replace(',', ".").parse::<f64>() {
        Ok(v) if (min..=max).contains(&v) => v,
        _ => fail(&format!("{what} invalide : {s}")),
    }
}

fn bank_or_fail(b: &str) -> Vec<presets::Preset> {
    presets::bank(b).unwrap_or_else(|| fail("banque attendue : ch1, ch2 ou reverb"))
}

fn main() {
    let o = parse_opts();
    let a: Vec<&str> = o.args.iter().map(String::as_str).collect();
    let run = |r: Result<(), Box<dyn std::error::Error>>| r.unwrap_or_else(|e| fail(&e.to_string()));

    // Chaque commande produit une liste de corps (commande ... F7) ou de messages complets.
    let msgs: Vec<Vec<u8>> = match a.as_slice() {
        ["list"] => return run(list()),
        ["monitor"] => return run(monitor(&o.port, 10)),
        ["monitor", s] => return run(monitor(&o.port, s.parse().unwrap_or_else(|_| fail("durée invalide")))),
        ["presets", b] => {
            for (i, p) in bank_or_fail(b).iter().enumerate() {
                println!("{:>2}. {}", i + 1, p.name);
            }
            return;
        }
        ["set", p, db] => {
            let param = Param::from_name(p).unwrap_or_else(|| fail("paramètre inconnu"));
            if param.is_pan() {
                fail("pour le PAN, utiliser la commande « pan »");
            }
            let db: f64 = db.parse().unwrap_or_else(|_| fail("valeur en dB invalide"));
            vec![set_gain(o.seq, param, db_to_gain(db))]
        }
        ["pan", ch, pos] => {
            let param = match *ch {
                "ch1" => Param::Ch1Pan,
                "ch2" => Param::Ch2Pan,
                _ => fail("canal attendu : ch1 ou ch2"),
            };
            let (l, r) = pan_gains(pos.parse().unwrap_or_else(|_| fail("position invalide")));
            vec![set_stereo(o.seq, param, l, r)]
        }
        ["switch", ch, state] => {
            let sw = match *ch {
                "ch1" => Switch::Ch1RevSend,
                "ch2" => Switch::Ch2RevSend,
                _ => fail("canal attendu : ch1 ou ch2"),
            };
            vec![set_switch(o.seq, sw, on_off(state))]
        }
        ["toggle", name, state] => {
            let b = toggle_body(name, on_off(state)).unwrap_or_else(|| fail("interrupteur inconnu"));
            vec![from_body(o.seq, &b)]
        }
        ["amp", state] => vec![from_body(o.seq, &amp_body(on_off(state)))],
        ["amp-type", t] => {
            let high = match *t {
                "high" => true,
                "low" => false,
                _ => fail("type attendu : high ou low"),
            };
            vec![from_body(o.seq, &amp_type_body(high))]
        }
        ["footswitch", f] => {
            let mute = match *f {
                "reverb" => false,
                "ch1-mute" => true,
                _ => fail("fonction attendue : reverb ou ch1-mute"),
            };
            vec![from_body(o.seq, &footswitch_body(mute))]
        }
        ["eq", ch, band, t, f, g, rest @ ..] => {
            let ch = channel(ch);
            let bi = eq::band_index(band).unwrap_or_else(|| fail("bande attendue : low, midlow, midhigh ou high"));
            let t = eq::BandType::from_name(t).unwrap_or_else(|| fail("type attendu : bell, lowshelf ou highshelf"));
            if (t == eq::BandType::LowShelf && bi != 0) || (t == eq::BandType::HighShelf && bi != 3) {
                fail("lowshelf : bande low seulement ; highshelf : bande high seulement");
            }
            let f = num(f, 20.0, 20000.0, "fréquence (20 à 20000 Hz)");
            let g = num(g, -18.0, 18.0, "gain (-18 à +18 dB)");
            let q = match rest {
                [] => 0.7,
                [q] => num(q, 0.1, 16.0, "Q (0,1 à 16)"),
                _ => fail("trop d'arguments"),
            };
            vec![from_body(o.seq, &eq::band_body(ch, bi, t, f, g, q))]
        }
        ["hpf", ch, f] => {
            let f = num(f, 20.0, 1000.0, "fréquence (20 à 1000 Hz)");
            vec![from_body(o.seq, &eq::hpf_body(channel(ch), f))]
        }
        ["comp", ch, t, r, g, a, rel] => {
            let ch = channel(ch);
            let t = num(t, -60.0, 0.0, "THRESH (-60 à 0 dB)");
            let r = if *r == "inf" { 1000.0 } else { num(r, 1.0, 1000.0, "RATIO (1 à 1000, ou inf)") };
            let g = num(g, 0.0, 18.0, "GAIN (0 à 18 dB)");
            let a = num(a, 0.1, 100.0, "ATTACK (0,1 à 100 ms)");
            let rel = num(rel, 0.0, 15800.0, "RELEASE (0 à 15800 ms)");
            [comp::threshold_body(ch, t, r, g), comp::attack_body(ch, a), comp::release_body(ch, rel)]
                .iter()
                .enumerate()
                .map(|(i, b)| from_body(o.seq.wrapping_add(i as u16) & 0x3FFF, b))
                .collect()
        }
        ["preset", b, q] => {
            let p = presets::find(bank_or_fail(b), q).unwrap_or_else(|e| fail(&e));
            println!("Preset : {}", p.name);
            p.msgs
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let body = presets::parse_hex(m).unwrap_or_else(|e| fail(&e));
                    from_body(o.seq.wrapping_add(i as u16) & 0x3FFF, &body)
                })
                .collect()
        }
        ["raw", body] => {
            let b = presets::parse_hex(body).unwrap_or_else(|e| fail(&e));
            if b.last() != Some(&0xF7) {
                fail("le corps doit se terminer par F7");
            }
            vec![from_body(o.seq, &b)]
        }
        _ => fail("commande inconnue"),
    };

    for m in &msgs {
        println!("Message : {}", hex(m));
    }
    if o.send {
        run(send_all(&o.port, &msgs));
    } else {
        println!("(non envoyé : ajouter --send pour l'envoyer au boîtier)");
    }
}
