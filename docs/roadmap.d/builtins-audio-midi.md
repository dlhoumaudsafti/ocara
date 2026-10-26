# Builtins audio : lecture/édition MIDI et enregistrement WAV, OGG, MP3

Statut : **à concevoir puis implémenter** — ouverte le 2026-10-26, priorité
moyenne. Rien n'est implémenté ; cette fiche décrit l'existant, le besoin et
les questions à trancher.

## Besoin

1. **MIDI** : lire un fichier `.mid` (pistes, événements note on/off, tempo,
   changements de programme, contrôleurs), l'éditer (ajouter, supprimer,
   transposer, quantifier des notes, changer le tempo) et l'enregistrer.
   À étudier : le jouer (synthèse via une SoundFont `.sf2`) et le
   **rendre** en fichier audio.
2. **Audio** : charger et créer des sons sous forme d'échantillons PCM,
   les éditer (couper, concaténer, mixer, volume, fondu) et les
   **enregistrer** en WAV, OGG (Vorbis) et MP3 ; à étudier : FLAC, Opus.
   Le tout **hors fenêtre** (sans `ocara.SDL`).

## Existant (vérifié)

- `ocara.SDL` (Palier 3, feature `mixer` de `sdl3`) sait **jouer** des
  fichiers WAV/OGG/MP3 (`loadSound`/`playSound`, `playMusic`…) dans une
  fenêtre ouverte. Il ne donne aucun accès aux échantillons, n'enregistre
  rien et ne gère pas le MIDI (libxmp est même désactivé, voir
  `runtime_sdl/Cargo.toml`).
- Aucun builtin ne lit ni n'écrit de fichier audio ou MIDI.

## API envisagée (à trancher)

```ocara
import ocara.Midi
import ocara.Audio

var song:Midi = Midi::load("song.mid")
for track in song.tracks() { ... }
song.track(0).addNote(tick: 480, note: 60, velocity: 100, duration: 240)
song.transpose(semitones: 2)
song.setTempo(bpm: 140)
song.save("out.mid")

var clip:Audio = Audio::load("voice.ogg")          // échantillons PCM décodés
var tone:Audio = Audio::create(sampleRate: 44100, channels: 2, seconds: 2.0)
clip.gain(0.5) / clip.fadeOut(seconds: 1.0) / clip.slice(...) / clip.mix(tone)
clip.save("out.wav")
clip.save("out.mp3", bitrate: 192)
clip.save("out.ogg", quality: 0.6)

var rendered:Audio = song.render(soundfont: "piano.sf2")   // à étudier
```

## Questions à trancher

- **Un builtin ou deux** (`ocara.Midi` + `ocara.Audio`) — deux paraît plus
  clair, le rendu MIDI → `Audio` faisant le lien.
- **Crates** : `midly` (lecture/écriture SMF, Rust pur) ; `hound` (WAV) ;
  `symphonia` (décodage multi-formats, Rust pur) ; encodage OGG Vorbis
  (`vorbis_rs`, liaison libvorbis) ; encodage MP3 (`mp3lame-encoder`,
  liaison LAME — **vérifier la licence LGPL** et la liaison statique) ;
  synthèse MIDI (`rustysynth`, Rust pur, SoundFont). Préférer le Rust pur
  pour garder la cross-compilation Windows/Android simple.
- **Crate séparé** (`runtime_audio`, comme `runtime_sdl`) pour ne lier les
  codecs que dans les programmes qui les importent.
- **Lecture temps réel** : rester sur `ocara.SDL` pour jouer, ou exposer un
  pont (`win.playAudio(clip)`) ; capture micro hors périmètre initial.
- **Erreurs** : `AudioException` / `MidiException`.
- **Mémoire** : buffers PCM volumineux — aucune copie implicite, libération
  par comptage de références, vérification des fuites
  (`ocaraunit --leak-check`).

## Critères d'acceptation

- `docs/builtins/Midi.md` et `docs/builtins/Audio.md`, procédure de
  `docs/adding-builtins.md` respectée.
- Aller-retour lire → éditer → enregistrer vérifié pour MIDI et pour chaque
  format audio annoncé (tests de régression avec fichiers de référence).
- Aucune fuite sur une boucle de création/destruction de clips.
- Exemples dans `examples/builtins/`.

## Complexité

Massive — un ou deux builtins complets, plusieurs codecs dont certains
liés à des bibliothèques C.
