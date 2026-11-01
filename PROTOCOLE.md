# AG03 MK2 : protocole de contrôle

Notes de rétro-ingénierie (31/10 et 01/11/2026). Outils : REA + Ghidra 12.1.4 (analyse statique),
Frida (capture en direct dans AG Controller V1.0.4.0, firmware AG03 MK2 V1.00).

## En résumé

- AG Controller pilote l'AG03 MK2 en **MIDI SysEx** via l'API MIDI standard de Windows (`winmm.dll`),
  port « Yamaha AG03MK2-1 ». Il n'utilise pas l'API bulk du driver Yamaha (voir annexe).
- Le port est **multi-clients** : un autre programme peut l'ouvrir pendant qu'AG Controller tourne.
- Chaque réglage = un message. Le boîtier **acquitte chaque message** (même numéro de séquence).
  Le numéro de séquence est libre (testé avec 1, 2, 3 : acceptés et appliqués).
- Les gains sont en virgule fixe **Q14** (16384 = 1,0 = 0 dB). Le compresseur, l'EQ, l'AMP SIM et la reverb
  sont envoyés sous forme de **coefficients DSP précalculés** par AG Controller.
- Le boîtier envoie en continu des **vumètres** (50 messages par seconde).
- **Testé en réel** : 3 messages générés par ag03ctl ont été acquittés et appliqués par le boîtier.

## Structure d'un message

```
F0 43 1n 3E 1C 78 | 00 | seq_lo seq_hi | cmd | données... | F7
```

| Octets      | Sens |
|-------------|------|
| `43`        | Fabricant Yamaha |
| `1n`        | `10` = envoi, `30` = réponse / requête |
| `3E 1C 78`  | Identifiant modèle (constant) |
| `00`        | Constant |
| `seq_lo seq_hi` | Compteur 14 bits (2 × 7 bits, poids faible d'abord) |
| `cmd`       | Type de commande |

### Commandes

| cmd  | Sens | Données |
|------|------|---------|
| `00` | Régler un paramètre | adresse (5 octets) + valeur |
| `30` | Régler un interrupteur | `10 nn` + état (`02` = on, `00` = off) + `00` |
| `50` | Réglage système (pédale) | `10 10` + valeur + `00` |
| `06` | Envoyé ~3 s après le dernier changement : bloc d'état de 40 octets (sauvegarde, contenu brouillé, non décodé) | bloc |
| `08` | Envoyé juste après `06` (validation) | `00` |
| `41` | (boîtier -> PC) vumètres, toutes les 20 ms | 42 octets |

Accusé de réception (boîtier -> PC) : `F0 43 30 3E 1C 78 00 seq_lo seq_hi cmd st 00 00 F7`,
avec `st` = `50` (commandes `00`, `30`, `50`, `06`) ou `40` (commande `08`).

## Encodage des valeurs

On lit les octets de valeur comme un entier, 7 bits par octet, poids faible d'abord :
`n = b0 | b1<<7 | b2<<14 | b3<<21 | b4<<28 | b5<<35`

- **Mono (3 octets)** : `gain = (n >> 4) / 16384`
- **Stéréo / paire (6 octets)** : `A = ((n >> 4) & 0xFFFF) / 16384`, `B = ((n >> 20) & 0xFFFF) / 16384`
  (PAN : gauche/droite ; interrupteurs du mode détaillé : direct/traité)

Exemples : 0 dB stéréo = `00 00 10 00 40 00` ; 0 dB mono = `00 00 10` ; coupé = tout à `00`.
PAN à puissance constante : centre 1,0/1,0 ; à fond d'un côté 1,414 (+3 dB) / 0.

## Table des paramètres

### Mode détaillé : page principale

| Réglage | Message (corps sans en-tête ni séquence) |
|---------|---------|
| CH1 PAN | `00 10 0E 05 04 00` + 6 octets stéréo |
| CH2 PAN | `00 10 11 05 04 00` + 6 octets stéréo |
| CH1 REV SEND | `00 10 09 04 00 00` + 3 octets mono |
| CH2 REV SEND | `00 10 0A 04 00 00` + 3 octets mono |
| Reverb LEVEL | `00 10 0C 05 04 00` + 6 octets stéréo |
| CH1 REV SEND on/off | `30 10 00 vv 00` (= **bouton REVERB de la table**) |
| CH2 REV SEND on/off | `30 10 01 vv 00` |

### Canal 1 (voix)

| Réglage | Adresse | Valeur |
|---------|---------|--------|
| COMP on/off | `10 39 00 04 00` | paire : off `00 00 10 00 00 00`, on `00 00 00 00 40 00` |
| COMP THRESH + RATIO + GAIN | `10 2D 00 08 00` | 8 octets |
| COMP ATTACK | `10 32 00 14 00` | 15 octets |
| COMP RELEASE | `10 30 00 00 00` | 3 octets |
| EQ on/off | `10 64 00 04 00` | paire |
| HPF on/off | `10 1F 00 04 00` | paire |
| HPF / point H | `10 15 00 24 00` | 24 octets |
| EQ bande 1 | `10 4B 00 24 00` | 24 octets |
| EQ bandes 2, 3, 4 | `10 55`, `10 5A`, `10 5F` + `00 10 00` | 12 octets |

### Canal 2 (instrument)

| Réglage | Adresse | Valeur |
|---------|---------|--------|
| COMP on/off | `10 48 00 04 00` | paire |
| COMP THRESH + RATIO + GAIN | `10 3C 00 08 00` | 8 octets |
| COMP ATTACK | `10 41 00 14 00` | 15 octets |
| COMP RELEASE | `10 3F 00 00 00` | 3 octets |
| EQ on/off (déduit) | `10 7F 00 04 00` | paire |
| HPF / point H | `10 21 00 24 00` | 24 octets |
| EQ bande 1 | `10 66 00 24 00` | 24 octets |
| EQ bandes 2, 3, 4 | `10 70`, `10 75`, `10 7A` + `00 10 00` | 12 octets |
| AMP on/off | interrupteur `30 10 03 vv 00` | |
| AMP HIGH / LOW | `10 09 01 08 00` | HIGH `70 7F 1F 00 00 66 01 00`, LOW `00 00 00 2A 02 00 00 00` |
| AMP GAIN | `10 0C 01 14 00` | 15 octets |
| AMP MASTER | `10 28 01 00 00` | 3 octets |
| AMP TREBLE + MIDDLE + BASS (un bloc) | `10 17 01 1C 00` | 44 octets |
| AMP PRESENCE | `10 2E 01 0C 00` | 17 octets |
| AMP OUTPUT | `10 45 01 00 00` | 3 octets |

### Reverb

| Réglage | Adresse | Valeur |
|---------|---------|--------|
| Bloc principal (INI DELAY, TIME...) | `10 79 04 1C 00` | 42 octets |
| Gros bloc (diffusion, filtres...) | `10 57 04 24 00` | 252 octets |
| Autres blocs | `10 46 04 08 00`, `10 49 04 08 00` | 8 octets |

Paramètres visibles : INI DELAY, TIME, DIFFUSION, HIGH DAMP, HPF, LPF (ce dernier caché sous la liste de presets,
il faut faire défiler).

### Système

| Réglage | Message |
|---------|---------|
| Pédale (Settings > Assignable foot switch) | `50 10 10 vv 00` : `00` = REVERB, `02` = CH1 MUTE ON/OFF |

## Calcul des coefficients (réglage libre)

Retrouvé par l'analyse des captures et la décompilation de AGController.exe. Vérifié à l'octet près.

### EQ et passe-haut (`src/eq.rs`)

- Biquads à 48 kHz, ordre `b0, b1, b2, -a1, -a2`. Gain arrondi au 0,5 dB.
- Cloche « boost/cut symétrique » : en boost num `1 ± αV`, den `1 ± α` ; en cut num `1 ± α`, den `1 ± α/V`
  (α = sin(w)/2Q, V = 10^(g/20)).
- Shelfs (type 1 grave, type 2 aigu) : Butterworth, K = tan(πf/fs) × √V (aigu) ou ÷ √V (grave) ;
  en cut, numérateur et dénominateur échangés.
- Passe-haut : RBJ du 2e ordre, Q = 1/√2.
- Bande 1 et passe-haut : 28 bits (Q26) ; mot fort de 16 bits (x >> 12) au bit 4+32i, mot faible de 12 bits
  au bit 20+32i ; arrondi simple. 108 bandes et 12 passe-haut identiques.
- Bandes 2 à 4 : 16 bits (Q14) au bit 4+16i ; b1 recalculé pour un gain exact de 1 à 0 Hz.

### Compresseur (`src/comp.rs`, fonction 0x140027700)

- Bloc seuil (8 octets), trois mots Q15 :
  `10^(-X/20)` avec X = tronc((1 - 1/R) × -THRESH) ; `10^(-Y/20)` avec Y = tronc(X brut - GAIN) ;
  `(1/R) × 0,5` (1/R tronqué à 4 décimales, 0 pour RATIO ∞). 27/27 identiques.
- Bloc attaque (15 octets) : passe-bas bilinéaire du 1er ordre, fc = tronc(1000 / (2π × ATTACK)) Hz entier,
  b0 = b1 = K/(1+K), a1 = (1-K)/(1+K), en 28 bits. 27/27 identiques (d'où ATTACK 27 = ATTACK 30).
- Bloc release (3 octets) : arrondi au pair de 15,8121745 / RELEASE(s), 65535 sous 0,241 ms.
  AG Controller arrondit le RELEASE affiché à ses propres crans (« 109 ms » envoie ~107 ms).

### Reverb (`src/reverb.rs`, en cours)

Blocs de la banque 04 : `46` (8 octets), `49` (8 octets), `79` (42 octets), `57` (252 octets).
Mesures brutes dans `notes/reverb/` (captures en faisant varier un seul réglage à la fois).

- **46 = HPF d'entrée, 49 = LPF d'entrée** : filtres bilinéaires du 1er ordre, trois mots Q14 [b0, b1, -a1]
  au bit 4+16i. LPF : b1 = 1 - a1 - b0 (gain exact à 0 Hz). 8/8 identiques. AG Controller arrondit
  les fréquences de ses presets (Hall 6700 -> 7000 Hz, Stage 10600 -> 11000 Hz, Room 85 -> 90 Hz).
- **79 = lignes de retard (TIME et HIGH DAMP)** : 18 mots de 16 bits = segment implicite de 8 mots,
  en-tête (641, 7), segment de 8 mots. Quatre groupes [x, g, g, c] : passe-bas dans la boucle de chaque
  ligne de retard (b0 = b1 = g, pôle c en Q14).
  - c (exact) : fc = 1920 × (HIGH DAMP + 3) Hz, pôle bilinéaire (1 - K) / (1 + K), K = tan(π fc / 48000).
  - G = 2g / (16384 - c) ne dépend que de TIME. -1/ln(G) suit une grille géométrique q = 1,041296
    (U0 = 3,95157 pour la 1re ligne à 2,8 s) ; les lignes 2, 3, 4 sont décalées de 4, 8 et 11 crans.
    AG Controller arrondit TIME sur cette grille (pas plus grossier au-delà de 5 s, voir `rev_time2.py`).
    ag03ctl utilise la loi continue, à moins de 1,5 % près.
  - x = 16384 × √((1 - G²) / 24).
  - TIME est stocké en dixièmes de seconde entiers.
- **57 = diffusion et pré-délai** : 110 mots organisés en segments (adresse, longueur n, puis n + 1 valeurs),
  voir `rev_57.py`. Contient six passe-tout (coefficients -g, 1 - g, 1 + g, g avec g de 0,658 à 0,711)
  et des pointeurs de lignes de retard.
  - DIFFUSION (10 -> 5) recule de 0, 52, 95, 106, 117, 122 les pointeurs 74, 81, 90, 97, et modifie 104, 106, 107.
  - INI DELAY déplace les pointeurs 104 à 107 (+264/+219 entre 0,1 et 10,9 ms), de façon non linéaire :
    pas encore modélisé. Ces deux réglages restent ceux du type de reverb dans l'appli.
- TIME varie de 0,1 s par cran au clavier ; les touches s'envoient par PostMessage à la fenêtre d'AG Controller.

### Encore à faire

- AMP SIM : modélisation de circuit (fonctions 0x1400295c0, 0x140028760, empaquetage 0x14002b2b0).
- Reverb : 4 blocs, dont un de 252 octets.

## Presets

Choisir un preset envoie seulement **les paramètres qui changent** (compresseur, EQ, HPF, AMP...,
et pour le canal 1 aussi le PAN, remis au centre, et le REV SEND). En rejouant toutes les captures dans l'ordre,
on reconstitue l'**état complet de chaque preset** (`presets/build_presets.py` -> `presets/presets.json`),
vérifié contre une capture indépendante.

- Canal 1 : 21 presets (Voice High / Middle / Low × Speech / Sing × micro main, statique, casque ;
  Acoustic / Instruments × 2 ; Default). 10 messages chacun.
- Canal 2 : 6 presets (E. Guitar Clean, Crunch, Distortion ; A. Guitar ; E. Bass ; Default). 17 messages chacun.
- Reverb : 4 presets (Default, Hall, Room, Stage). 4 messages chacun (le LEVEL n'est pas inclus).

## Modes d'AG Controller

- **Mode détaillé** (celui avec les courbes) : tout ce qui est décrit ci-dessus.
- **Mode Simple** (bouton en haut à droite) : un assistant pour le canal 1. Tessiture (Bass/Tenor, Alto, Soprano),
  type de micro (Dynamic, Condenser, HeadSet), Talk ou Sing, bouton Sound Check, interrupteur Reverb ON.
  Ces combinaisons correspondent aux presets voix (Low/Middle/High × micro × Speech/Sing).
  Passer en mode Simple **applique le preset choisi** (écrase le compresseur, le PAN et le REV SEND du canal 1,
  et rallume la reverb du canal 1). Revenir en mode détaillé n'envoie rien.

## Menu (en haut à gauche)

- **Console Files** : enregistre l'état dans un fichier CSV local
  (`%APPDATA%\Yamaha\AGController\scene_user_NN.csv`), sans rien lire sur le boîtier.
- **Settings** : langue (interface seulement) et fonction de la pédale (voir Système).
- **User Guide** : lien web.
- **License** : licences des bibliothèques tierces (Qt, ANGLE...).

## Reste à faire

1. Fait : les CSV (scènes et presets embarqués) contiennent les valeurs en clair.
2. Calcul des coefficients : fait pour l'EQ, le passe-haut et le compresseur ; reste l'ampli et la reverb.
3. Décoder les vumètres et le bloc de sauvegarde `06`.
4. Vérifier le port MIDI sous Linux.

## Annexe : API bulk du driver (non utilisée par AG Controller)

`ysusb_bulklib64.dll` : GUID `{677bb410-d9a0-4d16-9af2-8d2e23f247d6}`, IOCTL `0x222000` (infos),
`0x222028`, `0x222044` (ouvrir un port), `0x222048` (fermer), `0x22204C` (lire), `0x222050` (envoyer).
