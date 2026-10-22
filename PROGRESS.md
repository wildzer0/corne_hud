# Stato del progetto

Registro dei progressi per riprendere il lavoro tra una sessione e l'altra.
Specifica completa: [docs/PROMPT_HUD.md](docs/PROMPT_HUD.md). Protocollo: [docs/PROTOCOL.md](docs/PROTOCOL.md).
Aggiorna questo file alla fine di ogni sessione.

_Ultimo aggiornamento: 2026-10-22. Release **v0.2.0** (aggancio ai bordi, icona, combo/tap dance/key override, avvio automatico, backup con storico)._

---

## Milestone

| # | Milestone | Stato | Note |
|---|---|---|---|
| 1 | Analisi e protocollo | ✅ fatta | Protocollo HUD v1 in `docs/PROTOCOL.md` |
| 2 | Firmware | ✅ scritto, compila | **Mai provato sulla tastiera vera** |
| 3 | App base | ✅ fatta | Verificata con `--mock` e `--offline` |
| 4 | Live (stato, overlay, Vial, robustezza) | ✅ fatta | Verificata solo con `--mock` |
| 5 | Heads-up, etichette, ricerca, statistiche, HUD_TOG | ✅ fatta | Verificata solo con `--mock` |
| 6 | Distribuzione (CI, README) | ✅ fatta | CI verde sui tre sistemi (verificata dall'utente) |

Tutto è committato e pubblicato. Prima release: tag `v0.1.0`.

## Da fare (in ordine)

1. **Controllare la release v0.1.0** su GitHub: devono esserci i tre eseguibili, `SHA256SUMS.txt` e `keyboard-hud.example.toml`.
2. **Provare il firmware sulla tastiera**, flashato su entrambe le metà:
   - `firmware/tools/hud_probe.py info`, `state --watch` (Fn1, Fn2, tri-layer, `TG(4)`, `OSL(5)`, mod, Caps Word), `keymap --raw` prima e dopo `CMK_TOG`;
   - `keymap_version` deve aumentare dopo una modifica fatta in Vial;
   - Vial deve funzionare come prima;
   - assegnare `HUD_TOG` (USER18) da Vial e controllare che `hud_tog` aumenti.
3. **Provare l'app con la tastiera vera** (su Windows: da WSL la tastiera non è visibile):
   - connessione, riconnessione dopo scollegamento, pausa con Vial aperto e ripresa;
   - `HUD_TOG` tocco e doppio tocco, heads-up, click-through;
   - lucchetti dei layer: "tenuto" e "bloccato" sono dedotti dall'app (vedi decisioni), da verificare.
   - trascinamento dell'HUD su Windows dopo la correzione (vedi registro): clic tenuto fermo, clic rapidi, trascinamenti lunghi, anche su monitor con scala diversa;
   - aggancio ai bordi (anche con la taskbar in un lato diverso dal basso), menu → Posizione, ritorno sullo schermo all'avvio dopo aver scollegato un monitor;
   - backup: alla prima connessione deve nascere `backups/corne-v4_<data>.vil` con "Primo backup" in `CHANGELOG.md`; cambiare un tasto in Vial e chiuderlo deve produrre un nuovo backup con la riga giusta; **ricaricare un backup in Vial** (*File → Load saved layout*) e controllare che tutto torni (keymap, encoder, macro, combo, impostazioni); provare anche con la web app (pausa "un altro programma usa la tastiera");
   - combo, tap dance e key override: nel log deve comparire "Vial: N tap dance, N combo, N key override"; Shift tenuto deve mostrare ⌦ su ⌫; dopo una modifica in Vial e la chiusura di Vial l'HUD deve vedere le nuove voci;
   - avvio con il sistema: spuntare dal menu, controllare in `regedit` il valore `Keyboard HUD` sotto `HKCU\...\Run`, riavviare la sessione; poi togliere la spunta;
   - icona dell'`.exe` in Esplora risorse e in Proprietà → Dettagli (versione). La CI deve trovare `rc.exe`: se manca, nel log di build compare l'avviso "risorse Windows non incluse".
4. **File `LICENSE`** (GPL-2.0-or-later): manca ancora, anche se v0.1.0 è già pubblicata. In attesa di conferma dell'utente.
5. **Prossime release**: aumentare `version` in `app/Cargo.toml`, poi `git tag vX.Y.Z && git push origin vX.Y.Z`. Il tag deve corrispondere alla versione, altrimenti la CI si ferma.

## Piano delle prossime funzioni (concordato il 2026-10-22, in quest'ordine)

1. **Aggancio ai bordi e posizioni rapide** (idea 2): ✅ fatto, da provare su Windows.
   - [x] area di lavoro del monitor (su Windows con `GetMonitorInfo`, che esclude la taskbar; altrove una stima da `monitor_size`);
   - [x] aggancio a bordi, angoli e centro durante il trascinamento manuale (Windows), con `snap_distance` e `screen_margin` in `[appearance]`;
   - [x] menu "Posizione": griglia 3×3 per spostare l'HUD in un angolo, a metà di un bordo o al centro;
   - [x] l'HUD torna dentro lo schermo dopo un ridimensionamento e all'avvio (monitor scollegato);
   - [x] icona del progetto: finestra (tutti i sistemi), `.exe` (risorsa Windows), `.app` (macOS);
   - [x] metadati di versione nell'`.exe`; sezione "Windows: SmartScreen" nel README;
   - [ ] firma del codice per Windows: **rimandata dall'utente** (2026-10-22) (SignPath Foundation gratuita per progetti open source, che però richiede il file `LICENSE`; oppure Azure Artifact Signing o un certificato OV a pagamento).
2. **Combo, tap dance e key override di Vial** (idea 4): ✅ fatto, da provare con la tastiera vera.
   - [x] lettura via Raw HID (`vial_dynamic_entry_op`, senza sblocco) a ogni connessione, e dal `.vil` in offline;
   - [x] tap dance: tocco in grande, tenuto (o doppio tocco) in piccolo, tutte le azioni nel tooltip;
   - [x] combo: menu → "Mostra le combo" (ricordato nello stato), etichetta con l'output fra i tasti;
   - [x] key override dal vivo: con i modificatori giusti il tasto mostra il sostituto in colore accento;
   - [x] ricerca: output delle combo (tutti i tasti evidenziati), azioni dei tap dance, key override.
3. **Avvio automatico** (idea 6): ✅ fatto (`autostart.rs`), da provare su Windows e macOS. Voce di menu "Avvia con il sistema"; Windows con la chiave `Run` del registro, Linux con `~/.config/autostart/keyboard-hud.desktop`, macOS con un LaunchAgent. Verificato su Linux (WSLg/X11, con `XDG_CONFIG_HOME` nello scratchpad): la casella crea e rimuove il file.

4. **Backup automatico della configurazione** (proposta dell'utente, 2026-10-22), in quest'ordine:
   1. ✅ **pausa automatica sul traffico altrui** (`Device::take_foreign`/`listen`, `worker::foreign_pause`, silenzio richiesto 20 s): la web app di Vial (WebHID nel browser) non si vede tra i processi. Se l'HUD riceve risposte che non ha chiesto, va in pausa e riprende dopo alcuni secondi di silenzio. La pausa manuale resta consigliata per lavorare con la web app;
   2. ✅ **backup del `.vil`** (`vial/dump.rs`, `backup.rs`, `Device::dump`, `worker::run_backup`; keycode scritti come numeri, accettati da vial-gui) alla prima connessione, alla chiusura di Vial, alla fine del traffico altrui, al cambio di `keymap_version` e con un controllo periodico (circa 1 minuto, a tastiera ferma) per combo, tap dance e key override, che non aggiornano `keymap_version`;
   3. ✅ **storico con diff leggibile** (`changes.rs`, `backup::append_changelog`; lo scrive la UI, che conosce i nomi dei layer): `CHANGELOG.md` nella cartella dei backup, con voci come "NAV, riga 2 colonna 4 (Fn2 + B): Redo → Ctrl+Y" e "Combo aggiunta: J + K insieme → Esc".

   Dettagli del backup:
   - quando: alla prima connessione e soprattutto **alla ripresa dopo la pausa per Vial** (Vial chiuso = modifiche finite); si salva solo se l'impronta del contenuto è diversa dall'ultimo backup;
   - cosa: lo stesso formato di vial-gui (`version`, `uid`, `layout` con il keymap **grezzo** via VIA `0x12`, non quello con Colemak dell'HUD, `layout_options`, `macro`, `tap_dance`, `combo`, `key_override`, `alt_repeat_key`, `settings`, `encoder_layout`). Verificato in vial-qmk: tutte queste letture funzionano senza sblocco;
   - dove: cartella `backups/` accanto alla configurazione, file con data e ora (`corne-2026-10-22_0815.vil`), con un numero massimo da tenere (es. 30) e una voce di menu "Apri la cartella dei backup";
   - da verificare: che vial-gui ricarichi il file (*File → Load saved layout*). I nomi dei keycode vanno scritti come li scrive vial-gui; in alternativa i numeri, che vial-gui accetta;
   - da decidere: backup anche delle impostazioni QMK (`qmk_settings`, dipendono dalla versione del firmware).

5. **Dopo v0.2.0** (scelti dall'utente il 2026-10-22, in quest'ordine):
   1. ✅ **etichette per applicazione** (`foreground.rs`, `Labels::set_app`): `app = "code"` o una lista; confronto senza `.exe` e maiuscole; a parità di livello vince l'etichetta dell'app. Windows `GetForegroundWindow` + nome del processo, X11 `_NET_ACTIVE_WINDOW`/`_NET_WM_PID`/`/proc/<pid>/comm`, macOS `NSWorkspace` (verificato con `cargo check` in un crate di prova, perché l'app intera non compila per macOS da Linux), Wayland nessuno. Provato in WSLg/X11 con due istanze dell'HUD;
   2. **mini indicatore**: con l'HUD nascosto, un badge piccolo con lo stato "pericoloso" (layer bloccato, Caps Word, one-shot, modificatori bloccati);
   3. **icona nella tray**: pausa, mostra/nascondi, cartella dei backup, avvio con il sistema, esci;
   4. **statistiche più ricche**: bilanciamento tra le mani, carico per dito, uso di layer e combo, suggerimenti (sempre senza testo, sequenze o tempi).

Idee rimandate: modalità allenamento, esportazione PNG/SVG della keymap, finestra "Storico" per confrontare backup qualsiasi, interfaccia in inglese, WPM. Sconsigliate: ripristino dei backup dall'HUD (scrive sulla tastiera), analisi dei mod-tap (servono i tempi).

## Decisioni prese (non rimetterle in discussione senza motivo)

**Protocollo e firmware**
- Comando Raw HID `0xF0`, sottocomandi `GET_INFO`, `GET_STATE`, `GET_KEYMAP`. Intestazione `[0xF0, sub, stato, seq]`: la richiesta invia stato `0xFF`, e un buffer tornato invariato significa sblocco Vial in corso.
- `seq` è scelto dall'host e torna in eco, al posto di un contatore del firmware.
- La matrice dei tasti è **sempre esposta** (scelta dell'utente), anche senza sblocco Vial. È documentato nel README.
- `keymap_version` aumenta a ogni `CMK_TOG` e quando cambia l'impronta FNV-1a del keymap. L'impronta viene ricalcolata leggendo 8 posizioni per ciclo in `housekeeping`, perché vial-qmk non ha hook sulle scritture di Vial.
- `HUD_TOG` = `QK_KB_0 + 18` (`0x7E12`), ultimo dell'enum e di `customKeycodes`.
- In `keymap.c` la nuova sezione è la **9. HUD**; le vecchie 9 e 10 sono diventate 10 e 11.

**App**
- I file `.vil` usano i nomi di vial-gui (`KC_LSHIFT`, `FN_MO13`, `USER00`), non quelli QMK. Le tabelle sono generate da `build.rs` a partire da `app/data/keycodes/` (hjson di vial-qmk + `keycodes_v6.py` di vial-gui).
- Font di ripiego DejaVu Sans incorporato, per i simboli dei tasti. Il lucchetto è disegnato a mano perché nessun font ha il glifo.
- Il crate è dichiarato GPL-2.0-or-later, per i dati derivati da vial-qmk e vial-gui.
- Layer "tenuto" o "bloccato" vengono dedotti dai tasti premuti (`model::classify_layers`): il firmware dice solo quali layer sono attivi.
- Lo stato della finestra sta in `keyboard-hud-state.toml`, separato da `keyboard-hud.toml`, che non viene mai riscritto.
- Click-through: si attiva solo se il firmware ha `HUD_TOG`; si spegne da solo se la tastiera si scollega; non viene ricordato tra un avvio e l'altro.
- `HUD_TOG`, tocco singolo: con heads-up attivo fissa o libera l'HUD; senza heads-up lo mostra o nasconde. Il doppio tocco (300 ms) gestisce il click-through.
- Con le statistiche attive, il polling da nascosto è almeno 30 Hz. Le date delle statistiche sono in UTC.
- Mappa delle dita ricavata dalla matrice Corne: colonne 0–1 mignolo, 2 anulare, 3 medio, 4–6 indice, ultima riga di ogni metà pollici.
- Finestra senza barra delle applicazioni (`with_taskbar(false)`); Esc non chiude l'app; si esce dal menu contestuale.
- Il worker inoltra lo stato solo quando cambia, così a tastiera ferma l'HUD non ridisegna.
- Niente icona nella tray: la feature `tray` è stata tolta perché era vuota.
- Su Windows l'HUD sposta la propria finestra da solo (`GetCursorPos` + `OuterPosition`), invece di usare `StartDrag` di winit (vedi registro).
- Aggancio e posizioni rapide lavorano sulla finestra esterna (`outer_rect`) in punti egui. Su Windows l'area di lavoro è esatta (`GetMonitorInfo`), altrove è stimata da `monitor_size` supponendo monitor affiancati della stessa misura. Su Wayland la posizione non è nota: la griglia "Posizione" è disattivata.
- L'aggancio esiste solo dove l'HUD trascina da sé (Windows); con `StartDrag` (Linux, macOS) lo spostamento lo gestisce il sistema.
- Icona: sorgente unica `app/packaging/icon/make_icon.py` (numpy), che genera `icon.png`, `.ico`, `.icns` e `icon-64.rgba` (icona della finestra, incorporata con `include_bytes!`). I file generati sono committati: la CI non esegue Python.
- Tap dance, combo e key override (`vial/dynamic.rs`) si leggono solo a ogni connessione: `keymap_version` non li copre, ma Vial li modifica solo mentre è aperto, cioè con l'HUD in pausa, e alla ripresa la sessione riparte. Un firmware senza `vial_dynamic_entry_op` dà un insieme vuoto, non un errore.
- Key override sull'HUD: modificatori = `mods | oneshot_mods | oneshot_locked_mods`; il layer considerato è quello mostrato (QMK usa il layer da cui è stato letto il tasto). Le regole dei modificatori sono quelle di `key_override_matches_active_modifiers` di QMK.
- Combo: si disegnano solo se tutti i tasti sono sul layer mostrato; con tasti adiacenti solo l'etichetta, con tasti lontani anche le linee. Il mock ha tre combo dimostrative (J+K, X+C, D+F), oltre all'override della `.vil`.
- Avvio con il sistema: l'elemento è "attivo" solo se coincide esattamente con quello che questo eseguibile scriverebbe (percorso + opzioni `--x11`, `--vid`, `--pid`, `--verbose`); altrimenti è "da aggiornare" e la casella appare vuota. Lo stato si rilegge a ogni apertura del menu. Solo per l'utente corrente, mai voci di sistema.
- Backup: `.vil` con keycode **numerici** (vial-gui li accetta; la tabella `VIAL_NAMES` mescola nomi e costanti, quindi niente conversione inversa). Si confronta il JSON con l'ultimo backup dello stesso UID; i file si chiamano `<slug>_<YYYY-MM-DDTHHMMSSZ>.vil` e solo quelli vengono potati (`keep`, default 30). Il mock non fa backup. Impostazioni QMK: larghezze da `qmk_settings.c` (`dump::qmk_setting_width`), id sconosciuti saltati.
- Traffico altrui: durante il polling l'HUD invia solo richieste `0xF0`, quindi ogni report con un altro primo byte è di un altro programma. La pausa ascolta senza inviare e chiude la sessione dopo 20 s di silenzio.
- Il menu contestuale è una finestra separata, non un popup dentro l'HUD (vedi registro, 2026-10-22).

## Ambiente di sviluppo

- WSL2 su Windows. Esiste un display WSLg, ma OpenGL è in software (llvmpipe): il consumo di CPU qui non è rappresentativo.
- vial-qmk in `~/git/vial-qmk` (commit `dd43959`). La keymap è collegata con un symlink: `~/git/vial-qmk/keyboards/crkbd/keymaps/mia -> ~/dev/corne_hud/firmware/mia`. Prima puntava a se stesso; è stato corretto nella Milestone 2.
- Rust 1.96.0, fissato in `app/rust-toolchain.toml`.
- `gh` non è installato, quindi lo stato della CI si controlla dal sito.

## Comandi utili

```sh
# firmware
cd ~/git/vial-qmk && qmk compile -kb crkbd/rev4_1/standard -km mia -e SKIP_VERSION=yes

# app: gli stessi controlli della CI
cd app && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked
cargo run -- --mock                    # tastiera simulata
cargo run -- --offline --layer 4       # layer DEBUG da vial.json + .vil incorporati

# screenshot senza interazione (PPM; convertibile in PNG con un piccolo script Python + zlib)
./target/debug/keyboard-hud --mock --mock-at 11500 --screenshot /tmp/x.ppm   # DEBUG bloccato
#   altre opzioni: --search "(" , --heatmap all|layer, --stats-window, --menu, --screenshot-after MS
#   con --stats-window/--menu + --screenshot le finestre secondarie vengono incorporate nell'HUD
#   (in WSLg la cattura di una finestra separata non arriva mai)
# finestre reali in WSLg: avvia con --x11 e usa `xwininfo -root -tree`
```

Scenario di `--mock` (ciclo di 16 s): 0–2 s digitazione, 2–4 s Fn1 (SYM), 4–6 s Fn2 (NAV), 6–8 s tri-layer (SYS), 8–9,5 s one-shot ACC, 9,5–11 s Ctrl in mod-tap, 11–12,5 s Caps Word, 12,5–15 s DEBUG bloccato. Lo screenshot arriva dopo 2 s, quindi per catturare l'istante T usa `--mock-at T-2000`.

## Mappa del codice

- `firmware/mia/keymap.c`: sezione 9 (HUD). `vial.json`: `customKeycodes` + `HUD`.
- `app/src/hid/`: `protocol.rs` (formato), `device.rs` (richieste), `transport.rs` (hidapi, udev), `worker.rs` (thread, riconnessione, pausa Vial), `mock.rs`.
- `app/src/vial/`: `definition.rs`, `dynamic.rs` (tap dance, combo, key override), `keycode.rs`, `names.rs` (tabelle generate, `parse_keycode`), `legend.rs`, `vil.rs`.
- `app/src/`: `kle.rs`, `model.rs`, `config.rs`, `screen.rs` (geometria dello schermo), `autostart.rs` (avvio con il sistema), `labels.rs`, `headsup.rs`, `search.rs`, `stats.rs`, `clock.rs`, `logging.rs`, `paths.rs`, `cli.rs`, `i18n.rs` (tutte le stringhe utente).
- `app/packaging/`: `icon/` (icona e script che la genera), `macos/Info.plist`.
- `app/src/ui/`: `mod.rs` (app, overlay, menu), `header.rs`, `keyboard.rs`, `search_panel.rs`, `report.rs`, `theme.rs`.

## Registro delle sessioni

- **2026-10-22**: release **v0.2.0** (CI di `e36a328` verde prima del tag, controllata via API GitHub con `curl`). Punto 5.1: etichette per applicazione.

- **2026-10-22**: punto 4 completo, in tre commit. (1) Pausa sul traffico altrui (web app di Vial). (2) Backup automatico del `.vil`: letture `Device::dump` (keymap grezzo, encoder, macro decodificate dal buffer, voci dinamiche con gli slot vuoti, alt repeat, impostazioni QMK), `backup.rs` per file e potatura, trigger nel worker (connessione, cambio keymap, ogni 2 minuti a tastiera ferma). Formato verificato sui sorgenti di vial-gui (macro `["text"|"tap"|"down"|"up"|"delay", ...]`, keycode numerici accettati). (3) `CHANGELOG.md` leggibile. 104 test; prova completa con il mock (abilitando i backup solo per la prova): primo backup e poi tre modifiche descritte correttamente. **Da verificare con Vial vero il ricaricamento di un backup.**

- **2026-10-22**: punto 6 (avvio con il sistema) e nel piano il punto 4, backup automatico del `.vil` (proposta dell'utente; verificato in vial-qmk che keymap grezzo, macro, voci dinamiche e impostazioni si leggono senza sblocco). 93 test. Il codice Windows del registro è controllato solo con clippy; quello macOS solo dalla CI.

- **2026-10-22**: punto 4 del piano. Nuovo `vial/dynamic.rs` (strutture da 10 byte, regole dei key override come in QMK, lettura dal `.vil`), richieste in `protocol.rs`, `Device::dynamic()`, `Event::Layout` con `dynamic`, risposte del mock. UI: legenda dei tap dance, key override dal vivo, combo sull'HUD, ricerca estesa (`Hit::also` per i tasti delle combo). 88 test. Verificato con screenshot: combo sul layer BASE del mock, ricerca "ctrl+c" (prima la combo X+C, poi il tasto di NAV) e, forzando Shift per lo screenshot, ⌦ al posto di ⌫. Da provare sulla tastiera vera; la `.vil` dell'utente ha solo l'override Shift+⌫ = Canc.

- **2026-10-22**: aggancio ai bordi e posizioni rapide (nuovo `screen.rs`, con test), icona del progetto, risorse dell'`.exe` (`winresource` in `build.rs`; da Linux esce solo un avviso), icona del bundle macOS, sezione SmartScreen nel README. Verificato in WSLg/X11 con XTest: la griglia del menu sposta l'HUD nella posizione calcolata. Le coordinate X11 di WSLg includono una cornice finta, quindi la finestra esterna misura 976×497 invece di 900×400. Lo script dell'icona ricampiona con due `tensordot`: la prima versione con `einsum` ci metteva 2 min 45 s. Prossimo passo del piano: combo, tap dance e key override di Vial.

- **2026-10-22**: l'utente segnala che su Windows, dopo un po', l'HUD non si trascina più. Causa: in winit 0.30, `drag_window` alza il flag `dragging` e lo azzera solo con `WM_EXITSIZEMOVE`. Se Windows non avvia il ciclo di spostamento (bottone già rilasciato, oppure clic tenuto fermo: egui lo considera un trascinamento dopo 0,8 s), il flag resta alzato e ogni `StartDrag` successivo viene ignorato. Correzione: su Windows la finestra va alla posizione iniziale più lo spostamento del cursore letto con `GetCursorPos`. Il calcolo parte sempre dall'inizio del trascinamento, quindi gli errori non si sommano: un primo tentativo basato sugli spostamenti relativi del puntatore egui, provato in X11 con XTest, accumulava circa 60 px per trascinamento. Sugli altri sistemi resta `StartDrag`. Controllo con clippy anche per `x86_64-pc-windows-msvc` (target aggiunto a rustup). Da provare su Windows.

- **2026-10-22**: l'utente conferma che la CI è verde sui tre sistemi. Push dei commit del menu e di `PROGRESS.md`, poi tag e pubblicazione di **v0.1.0**. Da ora i commit li fa Claude, su `main`; push e tag solo su richiesta.

- **2026-10-22**: corretto il bug "menu tagliato con HUD piccolo". Il menu contestuale era un popup egui dentro la finestra dell'HUD: ora è una finestra separata (viewport immediata `hud-menu`), dimensionata sul contenuto e aperta accanto al clic, verso l'interno dell'HUD. Si chiude con ✕, Esc, perdita di focus o clic sull'HUD. Su Wayland la posizione dell'HUD non è nota, quindi il menu lo piazza il compositor. Anche il pannello di ricerca ora si restringe con la finestra. I comandi verso l'HUD passano da `root_cmd()`, perché dentro un'altra finestra `send_viewport_cmd` agirebbe su quella. Il log ora registra anche avvisi ed errori delle librerie. Verificato in WSLg/X11 con `xwininfo`: con l'HUD a 310×115 il menu esce come finestra a sé di 300×302. WSLg non dà il focus alle nuove finestre, quindi lì funzionano solo ✕ e il clic sull'HUD. Nota: una finestra secondaria aperta al primissimo fotogramma non viene creata, per questo `--menu` aspetta 1 s.

- **2026-10-21**: milestone 1–6 completate in un'unica sessione. Test: 77 unitari, tutti verdi. Verifica visiva con screenshot in modalità `--mock` e `--offline`. Restano aperti tutti i test su hardware e la prima esecuzione della CI.
