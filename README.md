# Keyboard HUD

Un HUD sempre in primo piano che mostra la mia **Corne v4.1** (firmware Vial) e la aggiorna in tempo reale: layer attivo con le sue legende, tasti premuti, modificatori, one-shot, Caps Lock / Caps Word, QWERTY o Colemak-DH, sistema operativo.

![Layer SYS con Fn1 + Fn2 tenuti](docs/img/tri-layer.png)

- **Portable**: un solo eseguibile per sistema, niente installazione e niente permessi di amministratore. Funziona anche da una chiavetta USB.
- **La fonte di verità è la tastiera**: l'HUD interroga il firmware via Raw HID, la stessa interfaccia di Vial. Niente hook di sistema, niente keylogger, nessun permesso di Accessibilità. Funziona anche su Wayland.
- **Nessun testo registrato**: l'HUD riceve solo quali tasti fisici sono premuti e su quale layer, mai i caratteri.

| | |
|---|---|
| ![Layer DEBUG con etichette semantiche](docs/img/debug.png) | ![Ricerca di "(": Fn1 + D](docs/img/search.png) |
| Layer DEBUG bloccato, con le etichette di VS Code | Ricerca: `(` si trova su SYM, con **Fn1 + D** |
| ![Heatmap](docs/img/heatmap.png) | ![Modalità offline](docs/img/offline.png) |
| Heatmap delle pressioni | Modalità offline (vial.json + .vil) |

---

## Indice

1. [Download](#download)
2. [Primo avvio](#primo-avvio)
3. [Firmware](#firmware)
4. [Uso](#uso)
5. [Linux: regola udev](#linux-regola-udev)
6. [Wayland](#wayland)
7. [Convivenza con Vial](#convivenza-con-vial)
8. [Configurazione](#configurazione)
9. [Statistiche e privacy](#statistiche-e-privacy)
10. [Modalità offline e simulata](#modalità-offline-e-simulata)
11. [Opzioni da riga di comando](#opzioni-da-riga-di-comando)
12. [Compilare l'app](#compilare-lapp)
13. [Risoluzione dei problemi](#risoluzione-dei-problemi)
14. [Licenza e crediti](#licenza-e-crediti)

---

## Download

Dalla pagina **Releases** del repository:

| Sistema | File |
|---|---|
| Windows 10/11 (x86_64) | `keyboard-hud-windows-x86_64.exe` |
| macOS 11+ (Apple Silicon e Intel) | `keyboard-hud-macos-universal.zip` (contiene `Keyboard HUD.app`) |
| Linux x86_64 (glibc, Ubuntu 22.04+, Fedora recente) | `keyboard-hud-linux-x86_64` |

Ogni release contiene anche `SHA256SUMS.txt` e `keyboard-hud.example.toml`.

## Primo avvio

1. Flasha il firmware con il supporto HUD su **entrambe** le metà (vedi [Firmware](#firmware)).
2. Metti l'eseguibile in una cartella qualsiasi e avvialo:
   - **Windows**: doppio clic. Se SmartScreen avvisa (l'eseguibile non è firmato), scegli *Ulteriori informazioni → Esegui comunque*.
   - **macOS**: estrai lo zip. La prima volta fai clic destro su `Keyboard HUD.app` → *Apri* (l'app è firmata ad-hoc, non da uno sviluppatore Apple registrato). In alternativa: `xattr -dr com.apple.quarantine "Keyboard HUD.app"`. L'app non compare nel Dock: si chiude dal menu contestuale dell'HUD.
   - **Linux**: `chmod +x keyboard-hud-linux-x86_64 && ./keyboard-hud-linux-x86_64`. Serve una regola udev, una sola volta (vedi [Linux](#linux-regola-udev)).
3. L'HUD compare in sovrimpressione. Con l'**heads-up** attivo (default) si nasconde da solo e ricompare quando tieni Fn1/Fn2, quando un one-shot è in attesa o quando tieni un modificatore.

**Dove finiscono i file.** Se la cartella dell'eseguibile è scrivibile, o contiene già `keyboard-hud.toml` (modalità portable), tutto sta lì accanto. Altrimenti si usa la cartella di configurazione dell'utente (`%APPDATA%\keyboard-hud\config`, `~/Library/Application Support/keyboard-hud`, `~/.config/keyboard-hud`). I file sono:

| File | Contenuto |
|---|---|
| `keyboard-hud.toml` | la tua configurazione (facoltativa) |
| `keyboard-hud-state.toml` | posizione, scala, opacità, vista, tema dell'ultima sessione |
| `keyboard-hud-stats.json` | contatori delle statistiche |
| `keyboard-hud.log` (+ `.log.1`) | log, massimo 1 MB per file |
| `cache/` | definizione Vial letta dalla tastiera |

## Firmware

Il firmware è la keymap Vial in `firmware/mia/`, per `crkbd/rev4_1/standard` nel fork [vial-qmk](https://github.com/vial-kb/vial-qmk). Il supporto HUD è la sezione **9. HUD (Raw HID)** di `keymap.c`; il protocollo è descritto in [docs/PROTOCOL.md](docs/PROTOCOL.md).

```sh
# una tantum: collega la keymap all'albero di vial-qmk
ln -s "$PWD/firmware/mia" ~/git/vial-qmk/keyboards/crkbd/keymaps/mia

cd ~/git/vial-qmk
qmk compile -kb crkbd/rev4_1/standard -km mia -e SKIP_VERSION=yes
```

Il file da flashare è `crkbd_rev4_1_standard_mia.uf2`. **Flasha entrambe le metà**: per ognuna tieni premuto il pulsante di reset (o usa `QK_BOOT` sul layer SYS), poi copia il file `.uf2` sull'unità che compare.

Dopo il flash, da Vial assegna **`HUD_TOG`** (tab *User*, ultimo tasto) dove preferisci, ad esempio su un tasto libero del layer SYS.

Il firmware funziona identico anche senza HUD collegato. Risponde solo alle richieste dell'HUD, non invia mai dati di sua iniziativa e non scrive nulla in EEPROM.

## Uso

| Azione | Come |
|---|---|
| Spostare l'HUD | trascinalo con il mouse |
| Ingrandire / rimpicciolire (50–200%) | Ctrl + rotellina sull'HUD, oppure menu → Scala |
| Menu (opacità, scala, vista, tema, heads-up, heatmap, statistiche, ricerca, click-through, pausa, esci) | clic destro sull'HUD |
| Mostrare / nascondere | un tocco di **HUD_TOG**. Con l'heads-up attivo il tocco *fissa* l'HUD a schermo, e un altro tocco lo restituisce all'heads-up |
| Click-through (i clic passano alle finestre sotto) | menu, oppure **doppio tocco** di HUD_TOG. Si disattiva solo con il doppio tocco |
| Cercare un tasto | Ctrl+F o `/` con l'HUD in primo piano, oppure menu → Cerca. Frecce per scegliere, Invio/Esc per chiudere |
| Sfogliare i layer (solo offline) | rotellina o tasti 0–9 |

Il click-through si attiva solo se la tastiera collegata ha il tasto HUD_TOG, così c'è sempre un modo per disattivarlo. Se la tastiera si scollega, si spegne da solo.

**Cosa mostra l'HUD**

- **Layer**: nome e colore (gli stessi dei LED: SYM azzurro, NAV verde, DEBUG rosso, ACC giallo). Il lucchetto indica un layer bloccato (`TG`); `1×` con pulsazione indica un one-shot in attesa. Gli altri layer attivi compaiono come etichette.
- **Legende**: quelle del layer attivo, già con Colemak-DH applicato dal firmware. I tasti trasparenti mostrano attenuata la legenda del layer sotto; i tasti layer hanno il colore del layer di destinazione; i mod-tap mostrano il tocco in grande e il modificatore in piccolo. Il tooltip dà il nome completo.
- **Modificatori**: Ctrl, Shift, Alt, Gui (Cmd/Opt su macOS). Pieno = tenuto, pallido = weak, contorno pulsante = one-shot, lucchetto = one-shot bloccato.
- **Caps Lock / Caps Word, QWERTY / Colemak-DH, OS** rilevato o forzato.
- **Tasti premuti**: si accendono subito e sfumano in 200 ms.

## Linux: regola udev

Su Linux l'accesso a `/dev/hidraw*` richiede una regola udev. È l'unico passaggio con `sudo`, da fare una volta. Se manca, l'HUD lo dice nel suo stato.

```sh
./keyboard-hud-linux-x86_64 --print-udev-rule
```

Stampa questa regola, la stessa consigliata da Vial (se Vial ti funziona già, è probabilmente già installata):

```
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{serial}=="*vial:f64c2b3c*", MODE="0660", GROUP="users", TAG+="uaccess", TAG+="udev-acl"
```

```sh
sudo nano /etc/udev/rules.d/59-vial.rules        # incolla la riga
sudo udevadm control --reload && sudo udevadm trigger
```

Poi scollega e ricollega la tastiera.

## Wayland

Su Wayland è il compositor a decidere la posizione delle finestre e se possono restare sempre in primo piano: molti (GNOME in particolare) ignorano entrambe le richieste. Se l'HUD finisce sotto le altre finestre o non ricorda la posizione, avvialo con:

```sh
./keyboard-hud-linux-x86_64 --x11
```

L'opzione forza XWayland, dove "sempre in primo piano" e la posizione funzionano. Anche il click-through dipende dal compositor.

## Convivenza con Vial

Vial e l'HUD usano la stessa interfaccia Raw HID. Le risposte destinate all'uno potrebbero finire nell'altro e confonderlo. Per questo:

- se l'HUD trova il processo **Vial** (o VIA) in esecuzione, chiude il dispositivo e mostra *"In pausa: Vial è aperto"*. Il controllo avviene ogni 2 secondi;
- quando Vial si chiude, l'HUD si ricollega da solo e **rilegge il keymap**, che potresti aver appena cambiato;
- dal menu c'è anche una **pausa manuale**.

L'HUD si accorge comunque dei cambi di keymap fatti da Vial. Il firmware rileva le modifiche in pochi millisecondi e l'HUD rilegge le legende. Fa lo stesso dopo `CMK_TOG` (QWERTY/Colemak-DH).

## Configurazione

Copia [`keyboard-hud.example.toml`](keyboard-hud.example.toml) accanto all'eseguibile con il nome `keyboard-hud.toml`, oppure nella cartella di configurazione dell'utente. Ogni voce è facoltativa. Le modifiche valgono **entro un secondo, senza riavviare**; solo `[device]` richiede un riavvio.

| Sezione | Cosa regola |
|---|---|
| `[device]` | `vid` / `pid` per scegliere la tastiera (default: la prima che risponde al protocollo HUD) |
| `[appearance]` | opacità (default 85%), scala, tema (`system`/`dark`/`light`), vista (`full`/`compact`), posizione iniziale |
| `[headsup]` | attivo, ritardo dei modificatori (400 ms), ritardo di scomparsa (800 ms), opacità da nascosto |
| `[[layers]]` | nome e colore di ogni layer, in ordine |
| `[[labels]]` | etichette personalizzate, per keycode (anche limitato a un layer) o per posizione |
| `[stats]` | statistiche attive, file, periodo del report "mai usati" |
| `[polling]` | frequenze di lettura (60 Hz visibile, 10 Hz nascosto) e timeout |
| `[log]` | livello del log |

Esempio di etichette (il file d'esempio contiene già tutto il layer DEBUG):

```toml
[[labels]]
layer = 4
keycode = "KC_F10"
label = "Step over"
description = "Esegui la riga (F10)"

[[labels]]
layer = 4
keycode = "C(KC_DOT)"      # anche S(...), A(...), C(S(...)), QK_KB_15 / USER15
label = "Quick fix"

[[labels]]                 # per posizione: layer, riga e colonna della matrice
layer = 0
row = 0
col = 0
label = "Debug"
```

Posizione, scala, opacità, vista, tema e heads-up scelti dal menu vengono ricordati in `keyboard-hud-state.toml`. Il tuo `keyboard-hud.toml`, con i suoi commenti, non viene mai riscritto.

## Statistiche e privacy

- Si conta solo **quante volte** viene premuto ogni tasto fisico, su quale layer e in quale giorno (UTC).
- **Mai** testo, sequenze di tasti o tempi tra un tasto e l'altro. I dati restano sul tuo computer, in `keyboard-hud-stats.json`, e non vengono mai trasmessi.
- Si disattivano con `[stats] enabled = false` e si azzerano dalla finestra delle statistiche, con conferma.
- **Heatmap** (menu → Heatmap): pressioni sul layer mostrato, oppure su tutti i layer insieme.
- **Finestra delle statistiche** (menu → Statistiche): utilizzo dei layer, distribuzione tra le mani e tra le dita, tasti mai usati negli ultimi N giorni, esportazione CSV (`keyboard-hud-stats.csv`).
- Il conteggio si basa sugli stati letti dalla tastiera. Con l'HUD visibile la lettura è a 60 Hz, da nascosto almeno a 30 Hz se le statistiche sono attive. Un tocco più breve di circa 16 ms può sfuggire.

**Nota sulla sicurezza.** Vial protegge la lettura della matrice dei tasti dietro lo sblocco. Il protocollo HUD invece la espone, perché serve proprio a questo. Qualunque programma locale che abbia accesso al dispositivo HID può quindi sapere quali tasti fisici sono premuti. Su Linux, con la regola udev consigliata, l'accesso è limitato all'utente collegato e al gruppo `users`.

## Modalità offline e simulata

- **Offline** (`--offline`, oppure automaticamente se la tastiera non c'è): mostra i layer da `vial.json` e da un file `.vil` esportato da Vial, senza stato live. I file si indicano con `--vial-json` e `--vil`, oppure si mettono accanto all'eseguibile (`vial.json` più il primo `*.vil`). I layer si sfogliano con la rotellina o con i tasti 0–9.
- **Firmware senza protocollo HUD**: l'HUD lo segnala, legge comunque il keymap via VIA e mostra i layer senza stato live.
- **Simulata** (`--mock`): una tastiera finta ripete in ciclo uno scenario realistico (digitazione, Fn1, Fn2, tri-layer, one-shot, Ctrl in mod-tap, Caps Word, DEBUG bloccato). Serve per sviluppare l'interfaccia senza tastiera.

## Opzioni da riga di comando

```
--mock                tastiera simulata
--offline             nessuna tastiera: vial.json + .vil dal disco
--vial-json PERCORSO  definizione Vial per la modalità offline
--vil PERCORSO        backup Vial (.vil) per la modalità offline
--vid 0xVVVV          usa solo dispositivi con questo Vendor ID
--pid 0xPPPP          usa solo dispositivi con questo Product ID
--x11                 Linux: forza XWayland
--print-udev-rule     stampa la regola udev per Linux ed esce
--layer N             layer mostrato all'avvio in modalità offline
--verbose             log dettagliato
--version, -h/--help
```

Ci sono anche opzioni di sviluppo, che servono a catturare gli stati dell'interfaccia: `--mock-at`, `--screenshot`, `--screenshot-after`, `--search`, `--heatmap`, `--stats-window`.

Su Windows l'eseguibile release è un'applicazione grafica senza console, quindi l'output di `--help` non si vede: le opzioni sono comunque quelle elencate qui.

## Compilare l'app

Serve Rust (la versione è fissata in `app/rust-toolchain.toml` e `rustup` la installa da sé). Non servono librerie di sistema: su Linux X11, Wayland e OpenGL vengono caricati all'avvio.

```sh
cd app
cargo run -- --mock          # prova con la tastiera simulata
cargo test                   # test unitari
cargo clippy --all-targets   # deve restare pulito
cargo build --release        # target/release/keyboard-hud
```

**Build delle release.** Il workflow [`.github/workflows/release.yml`](.github/workflows/release.yml) compila i tre eseguibili a ogni push e, a ogni tag `vX.Y.Z` (che deve corrispondere alla versione in `app/Cargo.toml`), pubblica la release con i file e i checksum:

```sh
git tag v0.1.0 && git push origin v0.1.0
```

Gli eseguibili Linux sono compilati su Ubuntu 22.04 per restare compatibili con le glibc più vecchie. Quello macOS è universale (arm64 + x86_64, `lipo`), impacchettato in un `.app` firmato ad-hoc.

**Struttura**

```
firmware/mia/          keymap Vial (sezione 9 = HUD), vial.json, corne.vil
firmware/tools/        hud_probe.py: test del protocollo da riga di comando
docs/PROTOCOL.md       protocollo Raw HID
app/src/hid/           trasporto hidapi, protocollo, worker, tastiera simulata
app/src/vial/          definizione Vial, keycode (tabelle generate), legende, .vil
app/src/kle.rs         parser KLE (rotazioni comprese)
app/src/model.rs       keymap effettivo, trasparenti, layer tenuti/bloccati, modificatori
app/src/ui/            overlay egui, intestazione, tastiera, ricerca, statistiche
app/src/{config,labels,headsup,search,stats}.rs
app/data/keycodes/     hjson di vial-qmk e nomi di vial-gui (fonte delle tabelle)
```

Le tabelle dei keycode sono **generate** da `build.rs` a partire dai file hjson di vial-qmk e da `keycodes_v6.py` di vial-gui, copiati in `app/data/keycodes/` (le versioni sono in `SOURCE.md`). Per aggiornarle basta ricopiare i file e ricompilare.

**Test del protocollo senza l'app**: `pip install hidapi`, poi `python firmware/tools/hud_probe.py info | state --watch | keymap --raw`, con Vial chiuso.

## Risoluzione dei problemi

| Sintomo | Causa e rimedio |
|---|---|
| "Ricerca della tastiera…" fisso | Cavo di sola alimentazione, oppure firmware senza HUD (ma allora compare un avviso). Prova `hud_probe.py info`. Su Linux controlla la regola udev |
| "Permesso negato sul dispositivo HID" | Linux: manca la regola udev, vedi sopra |
| "Firmware senza protocollo HUD" | Flasha il firmware di `firmware/mia/` su entrambe le metà |
| "In pausa: Vial è aperto" | Chiudi Vial: l'HUD riprende da solo |
| L'HUD non resta sopra le altre finestre (Linux) | Wayland: avvia con `--x11` |
| Non riesco più a cliccare l'HUD | Il click-through è attivo: doppio tocco di HUD_TOG. Riavviare l'app lo disattiva comunque |
| L'HUD è sparito | Heads-up: ricompare tenendo Fn1/Fn2. Oppure è nascosto: tocca HUD_TOG |
| Legende sbagliate dopo una modifica in Vial | Si aggiornano da sole entro un secondo. Altrimenti metti in pausa e riprendi dal menu |
| Dettagli | `keyboard-hud.log`, con `--verbose` per il livello debug |

## Licenza e crediti

- Codice dell'app: **GPL-2.0-or-later**. Le tabelle dei keycode derivano da [vial-qmk](https://github.com/vial-kb/vial-qmk) (GPL-2.0-or-later) e [vial-gui](https://github.com/vial-kb/vial-gui) (GPL-2.0).
- Firmware: basato su QMK/vial-qmk (GPL-2.0-or-later).
- Font: [DejaVu Sans](https://dejavu-fonts.github.io/) (licenza in `app/data/fonts/DejaVu-LICENSE.txt`), usato per i simboli dei tasti.
- Interfaccia: [egui/eframe](https://github.com/emilk/egui); accesso HID: [hidapi](https://github.com/ruabmbua/hidapi-rs).
