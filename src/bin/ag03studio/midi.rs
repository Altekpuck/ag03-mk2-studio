//! Connexion MIDI en tâche de fond : envoi des messages, accusés de réception, reconnexion automatique.

use ag03ctl::protocol::{from_body, parse_ack};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PORT: &str = "AG03";

#[derive(Clone, Default)]
pub struct Status {
    pub connected: bool,
    pub port: String,
    pub error: String,
    /// Incrémenté à chaque (re)connexion : l'interface renvoie alors tout l'état.
    pub generation: u64,
    pub sent: usize,
}

pub struct Midi {
    tx: Sender<Vec<Vec<u8>>>,
    pub status: Arc<Mutex<Status>>,
    pub acked: Arc<AtomicUsize>,
}

struct Link {
    out: MidiOutputConnection,
    _inp: MidiInputConnection<()>,
}

fn connect(acked: Arc<AtomicUsize>) -> Result<(Link, String), String> {
    let out = MidiOutput::new("AG03 MK2 Studio").map_err(|e| e.to_string())?;
    let port = out
        .ports()
        .into_iter()
        .find(|p| out.port_name(p).map(|n| n.contains(PORT)).unwrap_or(false))
        .ok_or("AG03 introuvable : vérifie le câble USB et le pilote Yamaha")?;
    let name = out.port_name(&port).unwrap_or_default();
    let out = out.connect(&port, "AG03 MK2 Studio").map_err(|e| e.to_string())?;

    let mut inp = MidiInput::new("AG03 MK2 Studio").map_err(|e| e.to_string())?;
    inp.ignore(midir::Ignore::None);
    let in_port = inp
        .ports()
        .into_iter()
        .find(|p| inp.port_name(p).map(|n| n.contains(PORT)).unwrap_or(false))
        .ok_or("entrée MIDI de l'AG03 introuvable")?;
    let inp = inp
        .connect(&in_port, "AG03 MK2 Studio", move |_, m, _| {
            if parse_ack(m).is_some() {
                acked.fetch_add(1, Ordering::Relaxed);
            }
        }, ())
        .map_err(|e| e.to_string())?;
    Ok((Link { out, _inp: inp }, name))
}

impl Midi {
    pub fn start(repaint: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let acked = Arc::new(AtomicUsize::new(0));
        let (s, a) = (status.clone(), acked.clone());
        std::thread::spawn(move || worker(rx, s, a, repaint));
        Midi { tx, status, acked }
    }

    pub fn send(&self, bodies: Vec<Vec<u8>>) {
        if !bodies.is_empty() {
            let _ = self.tx.send(bodies);
        }
    }

    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

fn worker(rx: Receiver<Vec<Vec<u8>>>, status: Arc<Mutex<Status>>, acked: Arc<AtomicUsize>, repaint: impl Fn()) {
    let mut link: Option<Link> = None;
    let mut seq: u16 = 1;
    loop {
        if link.is_none() {
            match connect(acked.clone()) {
                Ok((l, name)) => {
                    link = Some(l);
                    let mut s = status.lock().unwrap();
                    s.connected = true;
                    s.port = name;
                    s.error.clear();
                    s.generation += 1;
                }
                Err(e) => {
                    let mut s = status.lock().unwrap();
                    s.connected = false;
                    s.error = if e.contains("could not be initialized") { "MIDI indisponible sur ce système".into() } else { e };
                }
            }
            repaint();
        }
        match rx.recv_timeout(Duration::from_millis(if link.is_some() { 1000 } else { 2000 })) {
            Ok(bodies) => {
                // Les messages arrivés entre-temps partent avec ceux-ci
                let mut all = bodies;
                while let Ok(more) = rx.try_recv() {
                    all.extend(more);
                }
                if let Some(l) = link.as_mut() {
                    let mut failed = false;
                    let burst = all.len() > 4;
                    for b in &all {
                        if burst {
                            // envoi complet (connexion, preset) : on laisse respirer le boîtier
                            std::thread::sleep(Duration::from_millis(4));
                        }
                        if l.out.send(&from_body(seq, b)).is_err() {
                            failed = true;
                            break;
                        }
                        seq = (seq + 1) & 0x3FFF;
                    }
                    let mut s = status.lock().unwrap();
                    if failed {
                        link = None;
                        s.connected = false;
                        s.error = "connexion perdue".into();
                    } else {
                        s.sent += all.len();
                    }
                    drop(s);
                    repaint();
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                // Vérifie que le boîtier est toujours là
                if link.is_some() {
                    let present = MidiOutput::new("AG03 MK2 Studio check")
                        .map(|o| o.ports().iter().any(|p| o.port_name(p).map(|n| n.contains(PORT)).unwrap_or(false)))
                        .unwrap_or(false);
                    if !present {
                        link = None;
                        let mut s = status.lock().unwrap();
                        s.connected = false;
                        s.error = "AG03 débranché".into();
                        drop(s);
                        repaint();
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
