# Bloc 57 capturé en faisant varier seulement DIFFUSION (TIME 2,2 s, INI DELAY 0,1 ms, HIGH DAMP 8, HPF 8 kHz, LPF 11,2 kHz)
# Seuls ces fragments changent ; préfixe et suffixe identiques à la capture complète « 30 s » (rev_time.py n'a que le bloc 79).
PRE="00 10 57 04 24 00 20 66 04 19 13 48 57 26 5e 02 07 35 1c 54 4c 19 3d 66 44 66 6c 1a 33 09 26 60 01 00 7f 24 7f 13 11 30 5b 40 2d 40 4a 06 2a 02 58 36 60 5a 24 13 70 00 40 60 52 03 4b 78 53 6d 4f 16 72 25 4b 17 61 21 1b 07 2d 62 09 38 00 60 31 18 47 61 1c 06 73 18 4c 63 30 0e 43 39 0c 66 31 18 16 04 1c 00 20 55 41 00 04 5a 1a 00 40 00 30 04 2f 0d 04 46 60 55 01 0f 02 0e 00 20 28 61 30 04 05 15 08 46 00 7f 42 43 0a 72 2f 30 28 41 09 01 07 00 08 6e 30 7f 42 62 0d 74 2f 70 73 61 71 06 41 1e 18 6e 60 45 40 02 00 78 63 58 50"
MID1="0a 47 1e 74 47 61 68 03 1a 02 0a 00 10 15"
MID2="0e 5d 3a 7c 47 31 2a 08 7f 18 74 08 28 00 00 15 14 65 48"
MID3="52 2a 48 2a 28 28 05 01 24 20 01 00 7c 5c"
MID4="62 47 4b 2b 28 7a 39 31 05 05 04 01 05 00"
SUF="2e 52 3a 79 67 45 1f 17 f7"
# (valeur de DIFFUSION, fragments variables a, b, c, d, e) ; 10 = état avant le premier cran
D={10:("22 20","64 34","6c 1e","50 6d","28 74 32 50 0b 62 2e 0a 3b"),
   9:("22 2d","64 41","3c 20","20 6f","10 75 32 50 0b 55 2e 56 3a"),
   8:("22 38","24 4c","6c 21","50 70","18 75 32 50 0b 40 2e 52 3a"),
   7:("62 3a","24 4f","14 22","78 70","18 75 32 50 4b 51 2e 52 3a"),
   6:("62 3d","64 51","44 22","28 71","18 75 32 50 4b 4b 2e 52 3a"),
   5:("62 3e","24 53","54 22","38 71","18 75 32 50 4b 49 2e 52 3a")}
def block(v):
    a,b,c,d,e=D[v]
    return " ".join([PRE,a,MID1,b,MID2,c,MID3,d,MID4,e,"79 67 45 1f 17 f7"])
