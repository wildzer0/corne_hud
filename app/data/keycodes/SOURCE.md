# Sorgenti delle tabelle dei keycode

- `keycodes_*.hjson`: vial-qmk `data/constants/keycodes/`, commit `dd43959ae5c08d8a28d38a1acf7b04e86b14a344`
  (https://github.com/vial-kb/vial-qmk). Licenza GPL-2.0-or-later.
- `vial_keycodes_v6.py`: vial-gui `src/main/python/keycodes/keycodes_v6.py`, commit `aef8222a2d0429a183b2ed692d5f9efcfd383f08`
  (https://github.com/vial-kb/vial-gui). Licenza GPL-2.0. Serve solo per i nomi usati nei file `.vil`.

Per aggiornare: ricopia i file e ricompila (le tabelle sono generate da `build.rs`).
