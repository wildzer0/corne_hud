# Protocollo HUD v1 (Raw HID)

Trasporto: interfaccia Raw HID di VIA/Vial (usage page `0xFF60`, usage `0x61`), report da 32 byte.
Su tutte le piattaforme `hidapi` scrive 33 byte, con il report ID `0x00` in testa.
Modello richiesta/risposta: il firmware non invia mai report spontanei.
Tutti i campi multibyte sono **little-endian**; i byte riservati valgono 0.

Lato firmware: `raw_hid_receive_kb()` in `firmware/mia/keymap.c`, sezione 9. VIA chiama questa funzione
per i comandi che non conosce e poi rispedisce lo stesso buffer.

## Intestazione comune

| Byte | Richiesta | Risposta |
|---|---|---|
| 0 | `0xF0` | `0xF0`, oppure `0xFF` (`id_unhandled`) se il firmware non supporta l'HUD |
| 1 | sottocomando | sottocomando (eco) |
| 2 | `0xFF` | stato: `0` OK, `1` sottocomando sconosciuto, `2` argomento non valido |
| 3 | `seq` scelto dall'host | `seq` (eco), per scartare le risposte vecchie |
| 4–31 | argomenti | payload |

Se nella risposta lo stato vale ancora `0xFF`, il buffer è tornato senza modifiche. Succede quando Vial è nella fase
di sblocco (`vial_unlock_in_progress`): in quel caso VIA scarta tutti i comandi non-Vial. L'host deve riprovare più tardi.

## `0x01 GET_INFO`

| Byte | Campo |
|---|---|
| 4 | versione del protocollo HUD (1) |
| 5 | righe della matrice (8) |
| 6 | colonne della matrice (7) |
| 7 | numero di layer (6) |
| 8–9 | capacità: b0 Colemak, b1 Caps Word, b2 OS detection, b3 HUD_TOG, b4 Layer Lock, b5 split |
| 10–11 | keycode di `HUD_TOG` (`0x7E12` = `QK_KB_0 + 18` = `USER18`) |
| 12 | numero massimo di keycode per `GET_KEYMAP` (12) |
| 13 | `keymap_version` |

## `0x02 GET_STATE`

| Byte | Campo |
|---|---|
| 4–7 | `layer_state` (u32) |
| 8 | `default_layer_state` (u8, un bit per layer) |
| 9 | layer bloccati con `QK_LLCK` (bitmask) |
| 10–17 | matrice: byte *r* = riga *r*, bit *c* = colonna *c*. Righe 0–3 metà sinistra, 4–7 metà destra |
| 18 | `get_mods()` |
| 19 | `get_weak_mods()` |
| 20 | `get_oneshot_mods()` |
| 21 | `get_oneshot_locked_mods()` |
| 22 | layer one-shot attivo (`0xFF` = nessuno) |
| 23 | flag: b0 Caps Lock, b1 Caps Word, b2 Colemak, b3 DEBUG bloccato, b4 Num Lock, b5 slave connessa |
| 24 | OS rilevato: 0 incerto, 1 Linux, 2 Windows, 3 macOS, 4 iOS |
| 25 | modalità OS: 0 auto, 1 Windows, 2 macOS, 3 Linux |
| 26 | OS effettivo: 1 Windows, 2 macOS, 3 Linux |
| 27 | `hud_toggle_count`: +1 a ogni pressione di `HUD_TOG`, riparte da 0 dopo 255 |
| 28 | `keymap_version` |

Modificatori (byte 18–21), formato QMK a 8 bit: b0 LCtl, b1 LSft, b2 LAlt, b3 LGui, b4 RCtl, b5 RSft, b6 RAlt, b7 RGui.

## `0x03 GET_KEYMAP`

Richiesta: byte 4 = layer, 5–6 = offset (u16), 7 = count (1–12).
Risposta: byte 4–7 = eco della richiesta, 8–31 = `count` keycode (u16).

`offset` indica la posizione `riga × colonne + colonna`. I keycode vengono da `keymap_key_to_keycode()`,
quindi sono **effettivi**: con Colemak attivo il remap sui layer 0 e 5 è già applicato.
Se layer, offset o count sono fuori range, lo stato vale `2`.

## `keymap_version`

Aumenta di 1 quando cambia il keymap effettivo:
- a ogni `CMK_TOG`;
- quando Vial modifica un tasto. Vial non avvisa la keymap, quindi il master rilegge il keymap grezzo
  8 posizioni per ciclo e ne calcola un'impronta FNV-1a a 32 bit. Se a fine giro l'impronta è cambiata,
  la versione aumenta. Un giro completo dura pochi millisecondi.

Quando la versione cambia, l'HUD rilegge il keymap con `GET_KEYMAP`.

## Comandi VIA/Vial usati dall'HUD

| Comando | Uso | Note |
|---|---|---|
| `0xFE 0x00` | protocollo Vial (u32 LE) e UID (8 byte) | chiave della cache della definizione |
| `0xFE 0x01` | dimensione della definizione (u32 LE) | |
| `0xFE 0x02` + pagina (u16 LE) | 32 byte di definizione | JSON compresso xz |
| `0x02 0x02` | opzioni di layout (u32 **BE**) | per filtrare le alternative del KLE |
| `0x11` | numero di layer | |
| `0x12` | keymap grezzo (offset u16 **BE**, size ≤ 28, keycode **BE**) | senza Colemak; solo per confronto o in modalità degradata |
