"""Reconstitue l'état complet de chaque preset à partir des captures différentielles.

AG Controller n'envoie que les paramètres qui changent. En rejouant les captures dans l'ordre
chronologique à partir d'un état initial connu, on obtient l'état complet après chaque preset.
Sortie : presets.json  {"ch1": [{"name", "msgs"}], "ch2": [...], "reverb": [...]}
Chaque message est le corps après le compteur de séquence (commande ... F7).
"""
import json
from pathlib import Path

HERE = Path(__file__).parent


def key(msg: str) -> str:
    t = msg.split()
    if t[0] == "00":          # paramètre : commande + adresse (5 octets)
        return " ".join(t[:6])
    if t[0] in ("30", "50"):  # interrupteur : commande + adresse (2 octets)
        return " ".join(t[:3])
    return msg


def replay(captures, initial=None, skip=()):
    state = dict(initial or {})
    out = []
    for name, msgs in captures:
        for m in msgs:
            if key(m) in skip:
                continue
            state[key(m)] = m
        out.append({"name": name, "msgs": sorted(state.values())})
    return out


def load(path):
    rows = [json.loads(l) for l in open(HERE / path, encoding="utf-8")]
    return [(r["name"], r["msgs"]) for r in rows]


# Canal 1 : la 1re capture contient déjà les 10 paramètres que les presets modifient.
ch1 = load("ch1_log.jsonl")
ch1 = [(n.replace(" = preset d'origine", ""), m) for n, m in ch1]
ch1_presets = replay(ch1)

# Canal 2 : état initial (réglage d'origine) complété par déduction, voir PROTOCOLE.md.
ch2 = load("ch2_log.jsonl")
default2 = None
clean_extra = None
ch2_initial = {
    "00 10 66 00 24 00": "00 10 66 00 24 00 70 7d 0f 67 06 38 01 4c 78 41 6d 1f 7e 17 0c 7d 77 0e 40 2c 40 69 34 00 f7",
    "00 10 70 00 10 00": "00 10 70 00 10 00 30 7e 0f 3f 01 37 7c 11 68 4f 3c 60 f7",
    "00 10 75 00 10 00": "00 10 75 00 10 00 40 4e 10 37 1f 11 0e 11 09 0c 01 6d f7",
    "00 10 7a 00 10 00": "00 10 7a 00 10 00 00 00 50 67 33 47 6e 08 43 29 17 72 f7",
    "00 10 7f 00 04 00": "00 10 7f 00 04 00 00 00 10 00 00 00 f7",
    "00 10 09 01 08 00": "00 10 09 01 08 00 70 7f 1f 00 00 66 01 00 f7",
}
ch2_seq = ch2 + [
    ("Default", ["00 10 3c 00 08 00 50 03 04 45 19 00 40 00 f7", "00 10 41 00 14 00 50 00 00 61 05 0a 00 10 5c 20 7d 1f 7c 08 00 f7", "00 10 3f 00 00 00 10 10 00 f7", "00 10 66 00 24 00 70 7d 0f 67 06 38 01 4c 78 41 6d 1f 7e 17 0c 7d 77 0e 40 2c 40 69 34 00 f7", "00 10 70 00 10 00 30 7e 0f 3f 01 37 7c 11 68 4f 3c 60 f7", "00 10 75 00 10 00 40 4e 10 37 1f 11 0e 11 09 0c 01 6d f7", "00 10 7a 00 10 00 00 00 50 67 33 47 6e 08 43 29 17 72 f7", "00 10 21 00 24 00 30 68 0f 3c 08 70 05 7c 70 61 50 1f 78 10 10 74 07 48 03 3b 41 3f 31 00 f7", "00 10 48 00 04 00 00 00 10 00 00 00 f7", "00 10 7f 00 04 00 00 00 10 00 00 00 f7"]),
    ("E. Guitar / Clean", ["00 10 3c 00 08 00 30 18 54 0a 72 00 00 01 f7", "00 10 41 00 14 00 00 15 00 01 0c 50 02 10 40 41 2b 1f 7c 0f 00 f7", "00 10 3f 00 00 00 20 0b 00 f7", "00 10 21 00 24 00 40 76 0f 4d 05 2e 02 6c 4c 00 6d 1f 1a 0b 20 7b 47 33 01 4b 40 6d 3c 00 f7", "00 10 48 00 04 00 00 00 00 00 40 00 f7", "30 10 03 02 00 f7", "00 10 09 01 08 00 00 00 00 2a 02 00 00 00 f7", "00 10 0c 01 14 00 10 59 1f 2a 04 75 69 3b 18 4b 22 57 40 2b 01 f7", "00 10 28 01 00 00 30 58 04 f7", "00 10 17 01 1c 00 70 32 1c 5a 05 04 79 12 27 65 5d 60 07 7d 4d 7a 5f 15 40 4f 00 10 00 58 0c 20 4d 3f 0e 1b 48 13 57 0c 20 00 20 36 7e 0a 5d 7d 0b 09 f7", "00 10 2e 01 0c 00 60 16 00 25 7f 74 02 72 00 00 38 00 00 00 14 01 00 f7", "00 10 45 01 00 00 30 15 07 f7"]),
]
ch2_presets = replay(ch2_seq, ch2_initial)

# Reverb : 2e série de captures, en partant de l'état Default complet (1re série).
rev = {r["name"]: r["msgs"] for r in map(json.loads, open(HERE / "reverb_log.jsonl"))}
rev_initial = {key(m): m for m in rev["Default"]}
rev_seq = [("Stage", rev["Stage"]), ("Hall", rev["Hall"]), ("Room", rev["Room"]), ("Default", rev["Default"])]
# Le LEVEL (10 0C) n'est pas traité comme partie du preset : on garde le niveau de l'utilisateur.
rev_presets = replay(rev_seq, rev_initial, skip={"00 10 0c 05 04 00"})


def dedupe(presets):
    seen = {}
    for p in presets:
        seen[p["name"]] = p  # garde la dernière occurrence (état le plus sûr)
    return list(seen.values())


out = {"ch1": dedupe(ch1_presets), "ch2": dedupe(ch2_presets), "reverb": dedupe(rev_presets)}
(HERE / "presets.json").write_text(json.dumps(out, indent=1, ensure_ascii=False), encoding="utf-8")
for k, v in out.items():
    sizes = sorted({len(p["msgs"]) for p in v})
    print(f"{k}: {len(v)} presets, {sizes} messages chacun")
