## 0. Ruolo e obiettivo

Sei un ingegnere software senior con esperienza in **Rust**, **GUI desktop multipiattaforma**, **USB HID** e **firmware QMK**. Devi sviluppare **Keyboard HUD**: un'applicazione desktop **portable** (un solo eseguibile per sistema operativo, nessuna installazione, nessun privilegio di amministratore) che mostra sempre a schermo una rappresentazione grafica della mia tastiera split **Corne v4.1** e la aggiorna **in tempo reale**:

- layer attivo e relative legende dei tasti;
- tasti premuti;
- modificatori attivi, one-shot, Caps Lock / Caps Word;
- layout attivo (QWERTY o Colemak-DH, gestito dal firmware);
- sistema operativo rilevato dal firmware.

Il progetto ha **due parti**, entrambe a tuo carico:

1. **Firmware** (C, QMK/Vial): un piccolo protocollo su **Raw HID** con cui la tastiera comunica il proprio stato all'HUD.
2. **App desktop** (Rust): l'HUD vero e proprio.

Lavora per milestone (sezione 9). Alla fine di ognuna fermati, mostrami il risultato e aspetta la mia conferma prima di procedere. Se devi discostarti da una scelta di questo documento, chiedimelo prima spiegando il perché.

---

## 1. Perché l'architettura è questa (leggi prima di scrivere codice)

L'HUD **non** deve intercettare la tastiera a livello di sistema operativo (hook globali, keylogger, accessibilità). Motivi:

- non saprebbe in quale layer si trova la tastiera, né quale tasto **fisico** è stato premuto (il firmware rimappa i tasti tra layer e tra QWERTY/Colemak);
- richiede permessi speciali (Accessibilità su macOS) e su Wayland è di fatto impossibile;
- registrerebbe testo digitato: inaccettabile per la privacy.

La fonte di verità è quindi **la tastiera stessa**, interrogata via **Raw HID**: la stessa interfaccia USB usata da Vial (usage page `0xFF60`, usage `0x61`, report da **32 byte**). Il firmware risponde con lo stato reale: layer, matrice dei tasti premuti, modificatori, flag. Niente permessi speciali, funziona anche su Wayland, nessun testo registrato.

Il layout grafico e il keymap vengono **letti direttamente dalla tastiera** tramite il protocollo Vial/VIA (modalità "live"). I file `vial.json` e `.vil` servono solo per la modalità **offline** (vedi 5.9).

---

## 2. Materiale che ti fornisco (leggilo e verifica le ipotesi prima di iniziare)

- `keymap.c`, `config.h`, `rules.mk`, `rgb_matrix_user.inc`, `vial.json`: la mia keymap Vial per `crkbd/rev4_1/standard`, nel fork **vial-qmk** (`https://github.com/vial-kb/vial-qmk`).
- Un file `.vil` esportato da Vial con la mia configurazione attuale.

Contesto della mia keymap, che l'HUD deve rappresentare correttamente:

- **6 layer**: `0 BASE`, `1 SYM` (Fn1), `2 NAV` (Fn2), `3 SYS` (Fn1+Fn2, tri-layer), `4 DEBUG` (bloccabile con `TG(4)`), `5 ACC` (one-shot `OSL(5)`, vocali accentate).
- **Colemak-DH** non è un layer: è un rimappaggio delle posizioni fatto in `keymap_key_to_keycode()` sui layer 0 e 5, attivato da un tasto personalizzato (`CMK_TOG`) e salvato in EEPROM.
- **Tasti personalizzati** da `QK_KB_0` in poi (accentate, forzatura OS, tasti "intelligenti", `CMK_TOG`, `RGB_FUNZ`), con nomi definiti in `vial.json` → `customKeycodes`.
- Mod-tap `LCTL_T(KC_TAB)`, `QK_REP`, `CW_TOGG`, tasti `C(...)`, `S(...)`, `A(...)`.
- Il layout fisico in `vial.json` è in formato **KLE** (Keyboard Layout Editor), con **tasti dei pollici ruotati** (`r`, `rx`, `ry`) e la matrice 8 × 7 (righe 0–3 metà sinistra, 4–7 metà destra).
- La keymap usa già un canale RPC split personalizzato (`SPLIT_TRANSACTION_IDS_USER USER_SYNC_STATE`) e ha un indice di sezioni numerate in cima a `keymap.c`: **integra le tue modifiche rispettando quella struttura**, come nuova sezione, senza riscrivere il resto.

Prima di scrivere codice, verifica sui file reali e nei sorgenti di vial-qmk:

1. come sono serializzati i keycode nel `.vil` (stringhe tipo `"KC_A"`, `"LCTL_T(KC_TAB)"`, numeri, `"USER00"`…);
2. la struttura KLE in `vial.json`, comprese le rotazioni;
3. i comandi Raw HID di VIA/Vial (`quantum/via.h`, `quantum/via.c`, `quantum/vial.h`, `quantum/vial.c`) e come vengono instradati i comandi sconosciuti verso `raw_hid_receive_kb()`.

Riportami cosa hai trovato prima della Milestone 1.

---

## 3. Stack tecnologico (obbligatorio)

### App desktop: **Rust** (stable, edizione 2021 o successiva)
Motivo: un solo binario nativo, piccolo, senza runtime da installare, facile da compilare per Windows, macOS e Linux. È il modo più semplice di ottenere un software davvero **portable**.

Crate (usa le ultime versioni stabili e verifica le API sulla documentazione corrente):
- **`eframe` / `egui`**: GUI immediate-mode. Usa il backend `glow` (OpenGL), il più compatibile. Servono finestra **senza decorazioni**, **trasparente**, **sempre in primo piano** e **click-through** (`ViewportBuilder`: `with_decorations(false)`, `with_transparent(true)`, `with_always_on_top()`, `with_mouse_passthrough(...)`).
- **`hidapi`**: Raw HID. Su Linux usa il backend `hidraw`.
- **`serde`, `serde_json`, `toml`**: configurazione e parsing.
- **`lzma-rs`** (Rust puro): decompressione della definizione Vial (formato xz). Evita librerie C esterne.
- **`directories`**: percorsi di configurazione come ripiego.
- **`sysinfo`**: rilevare se Vial è in esecuzione (vedi 5.10).
- **Icona nella tray**: opzionale, dietro una feature Cargo. Su Linux richiede GTK e comprometterebbe la portabilità, quindi lì deve essere disattivata di default.

Vietato: Electron, Tauri, runtime esterni (Python, .NET, JVM), installer, servizi in background, driver.

### Firmware: **C** dentro la keymap QMK/Vial esistente
Nessuna modifica al core di vial-qmk: solo file della keymap (`keymap.c`, `config.h`, `rules.mk`).

---

## 4. Requisiti di portabilità (non negoziabili)

- **Un eseguibile per piattaforma**:
  - `keyboard-hud-windows-x86_64.exe`
  - `keyboard-hud-macos-universal` (x86_64 + arm64), in un `.app` firmato ad-hoc dentro uno zip
  - `keyboard-hud-linux-x86_64` (glibc, testato su Ubuntu 22.04+ e Fedora recente)
- **Nessuna installazione e nessun permesso di amministratore** per l'uso normale.
- **Modalità portable**: se nella cartella dell'eseguibile esiste `keyboard-hud.toml` (o la cartella è scrivibile), configurazione, statistiche e log stanno **accanto all'eseguibile**. Altrimenti si usa la cartella di configurazione utente del sistema. Deve funzionare da una chiavetta USB.
- **Linux**: l'accesso a `hidraw` richiede una regola udev. È l'unica eccezione che richiede `sudo`, una tantum. L'app deve:
  - rilevare l'errore di permesso e mostrare un messaggio chiaro con la regola esatta da copiare;
  - offrire un comando `keyboard-hud --print-udev-rule`;
  - documentarlo nel README.
- **Wayland**: "sempre in primo piano" e posizionamento della finestra dipendono dal compositor. Documenta il limite e offri l'opzione `--x11`, che forza XWayland.
- **Build riproducibile**: GitHub Actions che compila i tre target e pubblica gli artifact a ogni tag.

---

## 5. Funzionalità

### 5.1 Overlay HUD
- Finestra senza bordi, sfondo semitrasparente, opacità regolabile (default 85%), sempre in primo piano.
- **Click-through attivabile**: quando è attivo i clic passano alle finestre sotto. Si disattiva dal menu o dal tasto dedicato sulla tastiera (5.8).
- Trascinabile e ridimensionabile in scala (zoom 50–200%) quando il click-through è disattivo.
- Ricorda posizione, scala, monitor e opacità tra un avvio e l'altro. Supporta più monitor e DPI diversi.
- Due modalità di visualizzazione:
  - **Completa**: entrambe le metà, rese con la geometria reale (KLE con rotazioni).
  - **Compatta**: solo legende e layer, in una striscia più piccola.

### 5.2 Rendering della tastiera
- Geometria letta dalla definizione Vial (KLE): posizioni, dimensioni (`w`, `h`), rotazioni (`r`, `rx`, `ry`), etichette `"riga,colonna"`. Scrivi un parser KLE **con test unitari**.
- Per ogni tasto mostra la legenda del **layer attivo**, ottenuta dal keymap **effettivo** fornito dal firmware (6.3), quindi già con il rimappaggio Colemak applicato.
- Regole di visualizzazione:
  - `KC_TRNS`: mostra in grigio, attenuata, la legenda del primo layer sottostante non trasparente;
  - `KC_NO`: tasto vuoto e scuro;
  - **mod-tap** (es. `LCTL_T(KC_TAB)`): legenda doppia, tap in grande e hold in piccolo;
  - **layer key** (`MO`, `TG`, `OSL`, `TL_LOWR`/`TL_UPPR`): nome del layer di destinazione e il suo colore;
  - **modificatori combinati** (`C(KC_Z)`): forma leggibile, es. `Ctrl+Z`;
  - **tasti personalizzati**: nome breve da `customKeycodes` (`shortName`), nome completo nel tooltip.
- Tabella dei keycode: **generata da `data/constants/keycodes/*.hjson` di vial-qmk**, con uno script nel repository (o un `build.rs`), non scritta a mano. Tieni conto della versione del protocollo Vial/keycode riportata dalla tastiera.
- Legende con simboli compatti dove ha senso (`⌫`, `⏎`, `⇥`, `⇧`, frecce, `⎋`) e nome esteso nel tooltip.

### 5.3 Stato in tempo reale
- **Tasti premuti**: evidenziati con un'animazione breve (accensione immediata, dissolvenza ~200 ms).
- **Layer**: nome e colore ben visibili in un'intestazione. Colori di default uguali a quelli del firmware: SYM azzurro, NAV verde, SYS nessuno/neutro, DEBUG rosso, ACC giallo. Personalizzabili.
- **Layer bloccati** (`TG`): indicatore "lucchetto". **One-shot** (`OSL`, one-shot mods): indicatore "in attesa del prossimo tasto".
- **Modificatori attivi**: Ctrl, Shift, Alt, Gui (fisici, weak e one-shot, distinti graficamente).
- **Caps Lock / Caps Word**, **QWERTY / Colemak-DH**, **OS rilevato o forzato** (Windows, macOS, Linux, Auto).
- Latenza percepita sotto i 30 ms.

### 5.4 Comparsa automatica (modalità "heads-up")
Opzionale, attiva di default. L'HUD resta **nascosto o quasi trasparente** e compare con una dissolvenza quando:
- è attivo un layer diverso da BASE (anche solo tenendo Fn1/Fn2);
- è in attesa un one-shot;
- un modificatore resta premuto più di N ms (default 400 ms).

Scompare dopo N ms (default 800 ms) dal ritorno allo stato base. Con il layer DEBUG bloccato resta sempre visibile.

### 5.5 Etichette semantiche personalizzate
In `keyboard-hud.toml` posso ridefinire l'etichetta di qualsiasi tasto per layer e posizione, o per keycode. Esempio: sul layer DEBUG `F10` → "Step over", `F11` → "Step into", `S(KC_F5)` → "Stop", `C(KC_DOT)` → "Quick fix". Fornisci un file di esempio già compilato con le etichette del mio layer debug (ricavale da `keymap.c`).

### 5.6 Ricerca
Una scorciatoia (o il tasto dedicato, 5.8) apre una casella di ricerca: scrivo "è", "redo", "F10" o "{" e l'HUD evidenzia **in quale layer e su quale tasto** si trova, con la sequenza da premere (es. "Fn1 + D"). La ricerca copre nome del keycode, etichetta personalizzata e carattere prodotto.

### 5.7 Statistiche d'uso e heatmap (privacy prima di tutto)
- Conteggio delle pressioni **per posizione fisica e per layer**, salvato in locale.
- **Mai** registrare sequenze, testo o tempistiche tra tasti: solo contatori aggregati.
- Disattivabile; pulsante "azzera".
- Vista **heatmap** sovrapposta alla tastiera.
- Report: tasti mai usati negli ultimi N giorni, distribuzione tra le due mani e tra le dita, utilizzo di ciascun layer. Esportazione CSV.

### 5.8 Tasto dedicato sulla tastiera
Aggiungi al firmware un nuovo tasto personalizzato **`HUD_TOG`**, **in fondo** all'enum `custom_keycodes` e a `customKeycodes` in `vial.json` (l'ordine esistente non va toccato). Il firmware lo segnala all'HUD con un contatore che si incrementa a ogni pressione (6.2):
- tocco: mostra/nasconde l'HUD;
- tocco doppio entro 300 ms: attiva/disattiva il click-through.

Così non servono scorciatoie globali del sistema operativo, che richiederebbero permessi.

### 5.9 Modalità offline (senza tastiera o senza firmware compatibile)
- Carica `vial.json` e `.vil` dal disco: mostra tutti i layer, sfogliabili con mouse e rotellina o tasti 0–5. Niente stato live.
- Usala anche per sviluppare e testare l'interfaccia.
- Se la tastiera è collegata ma il firmware non supporta il protocollo HUD, mostra un avviso chiaro e passa automaticamente a questa modalità, leggendo comunque il keymap via VIA.

### 5.10 Convivenza con Vial
Vial e l'HUD usano la stessa interfaccia Raw HID. Le risposte dirette all'HUD potrebbero finire nel buffer di Vial e confonderlo. Quindi:
- se rilevi il processo Vial in esecuzione (`sysinfo`), **sospendi il polling e chiudi il dispositivo**, mostrando "In pausa: Vial è aperto";
- riprendi automaticamente quando Vial si chiude, **rileggendo il keymap**, che potrei aver appena modificato;
- prevedi anche una pausa manuale.

### 5.11 Aggiornamento del keymap
- Rileggi il keymap effettivo all'avvio, alla ripresa dopo Vial, quando cambia il flag Colemak e quando il firmware segnala un nuovo `keymap_version` (6.2).
- Nessun polling continuo del keymap completo.

### 5.12 Robustezza
- Collegamento e scollegamento a caldo della tastiera; riconnessione automatica.
- Funziona con entrambe le metà collegate al cavo: il master risponde sempre per tutta la tastiera.
- Log su file (rotazione, max 1 MB) con livello configurabile; opzione `--verbose`.
- Nessun panic in caso di dati malformati: valida ogni risposta HID.

### 5.13 Interfaccia e lingua
- Interfaccia in **italiano**, con stringhe centralizzate in modo da poter aggiungere l'inglese.
- Tema chiaro/scuro (segue il sistema, sovrascrivibile).
- Menu contestuale (clic destro, con click-through disattivo): opacità, scala, modalità, click-through, heads-up, statistiche, ricerca, pausa, esci.

---

## 6. Protocollo Raw HID (da implementare in firmware e app)

### 6.1 Principi
- Trasporto: interfaccia Raw HID di QMK, report da **32 byte**, usage page `0xFF60`, usage `0x61`. Su Windows `hidapi` richiede un byte di report ID `0x00` in testa alle scritture (33 byte): gestiscilo.
- Modello **richiesta/risposta** (polling dall'HUD). Il firmware **non** invia mai report spontanei, per non disturbare Vial.
- Comando di primo livello **`0xF0`**, non usato da VIA/Vial (VIA arriva a `0x13`, Vial usa `0xFE`, `0xFF` è "unhandled"). Verifica nei sorgenti che sia libero.
- Lato firmware: override di `raw_hid_receive_kb(uint8_t *data, uint8_t length)` nella keymap. VIA chiama questa funzione per i comandi sconosciuti e poi rispedisce lo **stesso buffer**: la risposta va scritta in `data`. **Non** chiamare `raw_hid_send()` lì dentro. Per comandi diversi da `0xF0` mantieni il comportamento originale (`data[0] = id_unhandled`).
- Tutti i campi multibyte sono **little-endian**. Ogni risposta riporta in `data[1]` il sottocomando e in `data[2]` uno stato (`0` = OK).

### 6.2 Sottocomandi
| Sottocomando | Richiesta | Risposta |
|---|---|---|
| `0x01 GET_INFO` | — | versione protocollo HUD (1), righe, colonne, numero di layer, bitmask di capacità (Colemak, Caps Word, OS detection, HUD_TOG) |
| `0x02 GET_STATE` | — | vedi sotto |
| `0x03 GET_KEYMAP` | layer, offset (u16), conteggio (≤ 12 keycode, per stare nei 32 byte insieme all'intestazione) | keycode **effettivi** (u16) a partire da `offset` nell'ordine `riga × colonne + colonna`, calcolati con `keymap_key_to_keycode(layer, (keypos_t){.row, .col})`, quindi **con il rimappaggio Colemak già applicato** |

Contenuto di `GET_STATE` (deve stare in 32 byte, definisci tu l'impaginazione esatta e documentala):
- `layer_state` (u32) e `default_layer_state` (u8 o u32);
- matrice dei tasti premuti: 8 righe × 7 colonne → 8 byte, un bit per colonna (`matrix_is_on()` o `matrix_get_row()` sul master, che ha anche le righe dell'altra metà);
- modificatori: `get_mods()`, `get_weak_mods()`, `get_oneshot_mods()`;
- layer one-shot attivo (se presente);
- flag: Caps Lock, Caps Word, Colemak, layer DEBUG bloccato;
- OS rilevato e modalità OS (auto/forzata);
- contatore `hud_toggle_count` (u8, incrementato da `HUD_TOG`);
- `keymap_version` (u8): incrementato quando cambia qualcosa che altera il keymap effettivo (toggle Colemak; se possibile anche scritture Vial sul keymap);
- un numero di sequenza (u8) per rilevare risposte vecchie.

### 6.3 Polling
- 60 Hz quando l'HUD è visibile o in modalità heads-up; 10 Hz quando è nascosto e non serve reattività; nessun polling in pausa.
- Ogni richiesta ha un timeout (es. 50 ms). Gli errori ripetuti fanno scattare la riconnessione.

### 6.4 Lettura di layout e keymap via Vial (modalità live)
- Definizione (geometria KLE e `customKeycodes`): `0xFE 0x01` (`vial_get_size`) e `0xFE 0x02` (`vial_get_def`, a pagine da 32 byte), poi decompressione xz e parsing JSON. Mettila in cache su disco, indicizzata per UID della tastiera (`0xFE 0x00`).
- Numero di layer e keymap "grezzo": `0x11` e `0x12` di VIA, se ti servono per confronto o per la modalità degradata. Per la visualizzazione usa `GET_KEYMAP` (6.2).

### 6.5 Vincoli firmware
- Nessuna scrittura in EEPROM causata dall'HUD.
- Tempo di gestione di ogni richiesta trascurabile: nessun ciclo lungo, nessun `wait_ms`.
- La keymap deve continuare a compilare e funzionare identica anche senza HUD collegato.
- Mantieni lo stile e i commenti in italiano di `keymap.c`. Aggiorna `vial.json` (`customKeycodes`) per `HUD_TOG`.
- Compila con `qmk compile -kb crkbd/rev4_1/standard -km mia -e SKIP_VERSION=yes` e ricordami di flashare **entrambe** le metà.

---

## 7. Configurazione dell'app (`keyboard-hud.toml`)

Includi un file d'esempio commentato con almeno:
- dispositivo: VID/PID opzionali (default: primo dispositivo con usage page `0xFF60` che risponde a `GET_INFO`);
- aspetto: opacità, scala, tema, modalità completa/compatta, posizione;
- heads-up: attivo, ritardi di comparsa e scomparsa;
- colori e nomi dei layer;
- etichette personalizzate (5.5);
- statistiche: attive, percorso;
- frequenze di polling.

Le modifiche al file vengono ricaricate senza riavviare l'app.

**Editor leggero della keymap** (aggiunto il 2026-10-23, scelta dell'utente): è l'unica cosa che l'HUD scrive sulla tastiera. Clic destro su un tasto → *Cambia questo tasto…*: ricerca nel catalogo dei keycode (QMK, tasti layer, tap dance, macro, keycode personalizzati) o keycode scritto a mano. Scrittura con VIA `dynamic_keymap_set_keycode`, solo se la tastiera conserva in quella posizione proprio il tasto mostrato (niente scritture attraverso il remap Colemak), dopo un backup riuscito, e riletta per conferma. Avvisi con conferma per layer non più raggiungibili, ultimo `QK_BOOT`, ultimo `HUD_TOG`, tasto vuoto sul layer base; `QK_BOOT` rifiutato. Annulla dell'ultima modifica dal menu. Macro, tap dance, combo, key override e impostazioni QMK restano a Vial.

**Storico dei backup** (2026-10-23): finestra con due backup a scelta (di default gli ultimi due della stessa tastiera), tasti cambiati evidenziati sulla tastiera disegnata per layer, elenco delle altre modifiche con le stesse frasi del `CHANGELOG.md`, *Rimetti com'era* per singolo tasto tramite l'editor. Il ripristino di un backup intero resta a Vial.

**Esportazione della keymap** (2026-10-23): pagina HTML stampabile in tema chiaro, un SVG per ogni layer non vuoto (stessa forma e legende dell'HUD, etichette personalizzate comprese), poi combo, tap dance, macro e key override a parole. Dal menu (salvata in `export/` e aperta nel browser) o con `--export FILE`. Il PNG si ottiene dal browser; un rasterizzatore nell'app non vale la dimensione dell'eseguibile.

**Modalità allenamento** (2026-10-23): finestra con un simbolo o una scorciatoia da premere, riconosciuta dagli stati della tastiera (tasto premuto risolto sui layer attivi; per i keycode con modificatori vale anche il tasto base con quei modificatori tenuti). Tasti dei layer sopra quello di default (tutti, se non ce ne sono), scelti a caso con peso maggiore per i poco usati (statistiche) e i sbagliati; mai lo stesso due volte di fila. Suggerimento dopo 6 s con la sequenza e il tasto illuminato sull'HUD; giri da 20; riepilogo con primo colpo, tempo medio e tasti da ripassare. Durante un giro l'HUD resta visibile anche con heads-up. Nulla viene salvato.

**Finestra Impostazioni** (stile VS Code): il file resta la fonte di verità, la finestra è una vista che lo modifica. Voci divise per sezione con descrizione, ricerca, indicatore "diversa dal predefinito" e ripristino; ogni modifica riscrive solo quella chiave con `toml_edit`, conservando commenti e formattazione. Si apre dal menu, dalla tray e con `Ctrl+,`; *Apri come testo* (`Ctrl+Shift+,`) apre il file nell'editor di sistema. Se il file non è valido la finestra è in sola lettura e mostra l'errore. Pagine dedicate per `[[layers]]` (nome, colore con selettore, aggiunta e rimozione dell'ultimo) e `[[labels]]` (testo, descrizione, keycode o posizione, layer, app, con la validazione sulla scheda), e una scheda *File di testo* che modifica il file intero e lo salva solo se valido.

Anche le scelte del menu (scala, opacità, vista, tema, lingua, stile, tasti, heads-up, combo, keycast) vengono scritte nel TOML allo stesso modo; `keyboard-hud-state.toml` ricorda solo la posizione della finestra.

---

## 8. Qualità

- Architettura a moduli separati: `hid` (trasporto e protocollo), `vial` (definizione, keycode), `kle` (geometria), `model` (stato), `ui` (egui), `stats`, `config`.
- **Dispositivo simulato** (`--mock`): genera stati realistici (cambi di layer, tasti premuti, one-shot) per sviluppare e testare l'interfaccia senza tastiera.
- Test unitari almeno per: parser KLE (comprese le rotazioni), decodifica dei keycode (inclusi mod-tap, layer e custom), codifica/decodifica del protocollo, risoluzione dei trasparenti, gestione dei dati malformati.
- `cargo clippy` e `cargo fmt` puliti; nessun `unsafe` salvo dove imposto dalle librerie.
- **README in italiano**: download, primo avvio, regola udev su Linux, Wayland, convivenza con Vial, configurazione, privacy delle statistiche, compilazione del firmware e dell'app.
- Codice e commenti dell'app in inglese; commenti del firmware in italiano, come il resto di `keymap.c`.

---

## 9. Milestone

1. **Analisi e protocollo** — Verifica le ipotesi (sezione 2), proponi l'impaginazione definitiva dei messaggi e la struttura del progetto. *Fermati e attendi conferma.*
2. **Firmware** — Implementa `raw_hid_receive_kb`, `GET_INFO`, `GET_STATE`, `GET_KEYMAP`, `HUD_TOG`, aggiorna `vial.json`. Fornisci uno script di test minimo (anche Python, solo per sviluppo) che interroghi la tastiera e stampi lo stato. *Fermati: lo provo sulla tastiera.*
3. **App base** — Connessione HID, lettura della definizione Vial, parser KLE, rendering statico della tastiera, modalità offline e `--mock`.
4. **Live** — Stato in tempo reale (5.3), legende e trasparenti (5.2), overlay (5.1), riconnessione e convivenza con Vial (5.10–5.12).
5. **Funzioni avanzate** — Heads-up (5.4), etichette personalizzate (5.5), ricerca (5.6), statistiche e heatmap (5.7), tasto `HUD_TOG` (5.8).
6. **Distribuzione** — Build dei tre eseguibili portable, GitHub Actions, README, file di configurazione d'esempio.

---

## 10. Criteri di accettazione

- Avvio l'eseguibile da una cartella qualsiasi (anche da una chiavetta), senza installare nulla, e vedo la mia Corne con la forma corretta, pollici ruotati compresi.
- Tenendo premuto Fn1, Fn2 o entrambi, l'HUD mostra il layer giusto entro una frazione di secondo; rilasciando torna al layer base.
- Bloccando il layer DEBUG compare il lucchetto e le etichette semantiche ("Step over"…).
- Dopo `CMK_TOG` le legende del layer base mostrano Colemak-DH, senza che l'HUD contenga una propria copia della tabella di rimappaggio.
- I tasti premuti si illuminano; modificatori, Caps Word e one-shot sono visibili.
- Con Vial aperto l'HUD va in pausa da solo, e alla chiusura di Vial riprende con il keymap aggiornato.
- Nessun testo digitato viene mai salvato o trasmesso.
- Funziona su Windows 11, macOS recente (Apple Silicon e Intel) e Linux (X11 e, con i limiti documentati, Wayland).
