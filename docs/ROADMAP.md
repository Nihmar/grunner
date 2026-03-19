# Grunner — Roadmap feature QoL

---

## 📋 Prompt 0 — Audit e refactoring strutturale della codebase

Devo fare un'**analisi approfondita della struttura del codice sorgente di Grunner** prima di procedere con qualsiasi nuova feature, per identificare problemi architetturali, accoppiamenti stretti, e funzioni troppo complesse che renderebbero difficile l'estensione futura.

**Obiettivo dell'analisi:**
Produrre un report dettagliato che vada file per file, identificando:

**1. Accoppiamenti stretti (tight coupling)**
- Moduli che dipendono direttamente da dettagli implementativi di altri moduli invece che da interfacce/trait
- Funzioni che ricevono widget GTK concreti dove invece basterebbero callback o trait object
- Stato globale condiviso in modo implicito

**2. Funzioni troppo lunghe o con troppe responsabilità**
- Funzioni che fanno più di una cosa e andrebbero spezzate
- In particolare `build_ui` in `ui.rs` che sospetto sia molto lunga — va analizzata e proposta una suddivisione logica
- Logica di business mescolata con logica di presentazione

**3. Moduli che potrebbero beneficiare di un'interfaccia trait**
- Tutto ciò che è un "provider" di risultati (app launcher, file search, Obsidian, shell, calculator) dovrebbe probabilmente implementare un trait comune per rendere semplice aggiungerne di nuovi
- Event handler e callback che potrebbero essere standardizzati

**4. Duplicazioni di codice**
- Pattern ripetuti che andrebbero estratti in funzioni helper condivise
- CSS class management, clipboard handling, D-Bus calls

**5. Punti critici per le feature future**
- Identificare dove andrà inserita la strip preferiti (Prompt 1) e quanto sarà invasivo
- Identificare dove andranno inseriti i context menu (Prompt 2-6) e se serve refactoring preventivo
- Valutare se il sistema di config è sufficientemente estensibile o va ristrutturato prima

**Output atteso:**
Un report in Markdown strutturato così:
- Una sezione per ogni file sorgente con: responsabilità attuali, problemi identificati, proposte concrete di intervento
- Una sezione finale con la lista prioritizzata degli interventi consigliati prima di procedere con le nuove feature
- Eventuale proposta di nuova struttura a moduli se quella attuale è insufficiente

**File da allegare:** **tutti i file `.rs` del progetto** e `style.css`.

---

## ✅ Già implementato in sessione

- Edge trigger + sidebar workspace con hover reveal
- Fade bordo destro workspace bar
- Fix hover power bar buttons
- Fix focus entry all'avvio

---

## 📋 Prompt 1 — Strip applicazioni preferite

Devo aggiungere a Grunner una **riga orizzontale di applicazioni preferite** (pinned apps) posizionata tra la search entry e la lista risultati.

**Comportamento:**
- Visibile solo quando l'entry è **vuota** — scompare non appena l'utente inizia a digitare
- Visibile solo se ci sono **applicazioni pinnate** — assente se la lista preferiti è vuota
- Il separatore sotto la strip segue la stessa visibilità della strip stessa
- Le app preferite **compaiono comunque nella lista normale** in qualsiasi momento — la strip è solo un accesso rapido, non le esclude dalla ricerca né le "cattura" in modo esclusivo

**Aspetto:**
- Riga di sole icone 32px, no label, no background sui singoli elementi
- Separatore sottile sotto, identico a quelli già usati in Grunner
- Transizione hide/show coerente con il resto dell'UI

**Interazione:**
- Click sinistro — apre l'app
- `Alt+1` … `Alt+9` — apre la N-esima app preferita (max 9 slot)
- Tasto destro — menu contestuale con: **Apri**, **Rimuovi dai preferiti**

**Config e persistenza:**
- La lista dei preferiti viene salvata nella config esistente come lista di app ID (desktop entry ID)
- Al riavvio le preferite vengono ricaricate e la strip ripopolata
- Le preferite si aggiungono tramite il context menu della lista risultati (implementato separatamente nel Prompt 2)

**Note implementative:**
- La strip usa un `gtk::Box` orizzontale con visibilità gestita tramite `connect_changed` sull'entry — stesso pattern già usato per `obsidian_bar` e `command_icon`
- `Alt+N` gestito nel keyboard controller esistente in `ui.rs`

**File da allegare:** `ui.rs`, `style.css`, `config.rs` e gli altri file rilevanti per la gestione della config e del layout.

---

## 📋 Prompt 2 — Context menu lista risultati

Devo aggiungere a Grunner un **menu contestuale (tasto destro) sui risultati della lista app**.

**Voci del menu:**
- **Apri** — stessa azione del doppio click / Enter
- **Aggiungi ai preferiti** — aggiunge l'app alla lista preferiti (se non già presente)
- **Rimuovi dai preferiti** — visibile solo se l'app è già nei preferiti, la rimuove
- **Apri come amministratore** — lancia l'app tramite `pkexec`
- **Apri percorso** — apre Nautilus nella cartella dell'eseguibile
- **Copia comando** — copia la stringa `Exec` del `.desktop` negli appunti

**Note implementative:**
- Il menu deve essere un `gtk::PopoverMenu` o `gio::Menu`
- Le voci "Aggiungi/Rimuovi dai preferiti" sono mutualmente esclusive in base allo stato attuale
- Il sistema preferiti sarà implementato separatamente — per ora basta che il menu chiami una funzione stub `toggle_pinned(app_id)`
- Deve funzionare sia in modalità normale che nelle altre modalità (file, calcolatrice, ecc.) — in quei casi mostrare solo "Apri"

**File da allegare:** `ui.rs`, `list_model.rs`, `style.css` e gli altri file rilevanti.

---

## 📋 Prompt 3 — Context menu workspace bar

Devo aggiungere a Grunner gestione avanzata delle finestre nella **workspace bar**.

**Funzionalità richieste:**

**1. Tasto destro sui bottoni della workspace bar**
Ogni bottone che rappresenta una finestra aperta deve rispondere al tasto destro con un popover/menu contestuale con tre voci:
- **Apri** — stessa azione del click sinistro (porta la finestra in primo piano)
- **Chiudi** — chiude la finestra tramite D-Bus
- **Chiudi tutte le finestre** — chiude tutte le finestre della workspace corrente tramite D-Bus

**2. Badge di chiusura on-hover**
Quando il mouse passa sopra un bottone della workspace bar, appare un cerchietto con `×` nell'angolo in alto a destra del bottone. Cliccarlo chiude quella singola finestra. Sparisce quando il mouse lascia il bottone.

**3. Pulsante "Chiudi tutte" in fondo alla barra**
Un piccolo pulsante con icona `window-close-symbolic` posizionato come ultimo elemento della riga della workspace bar. Visibile solo quando ci sono finestre aperte. Chiude tutte le finestre della workspace corrente tramite D-Bus. Stessa funzione condivisa con la voce del menu contestuale del punto 1.

**Note implementative:**
- Il badge `×` deve essere un overlay sul bottone, non spostare il contenuto
- Le azioni D-Bus di chiusura finestra sono già implementate nel codebase — riutilizzarle
- Il menu contestuale deve essere un `gtk::PopoverMenu` o `gio::Menu`, non un dialogo

**File da allegare:** `workspace_bar.rs`, `ui.rs`, `style.css` e gli altri file rilevanti per la gestione D-Bus delle finestre.

---

## 📋 Prompt 4 — Context menu ricerca Obsidian

Devo aggiungere a Grunner un **menu contestuale (tasto destro) sui risultati della ricerca Obsidian**.

**Voci del menu:**
- **Apri in Obsidian** — stessa azione del doppio click / Enter
- **Copia percorso nota** — copia il path assoluto del file `.md` negli appunti
- **Copia contenuto nota** — legge l'intero file `.md` e ne copia il testo negli appunti (i file Markdown sono sempre abbastanza piccoli da non preoccuparsi della memoria)
- **Apri in editor di testo** — apre il file `.md` con l'app di default del sistema
- **Mostra nel file manager** — apre Nautilus nella cartella della nota

**Note implementative:**
- Il menu appare solo quando la modalità attiva è Obsidian (`:ob`)
- `Copia contenuto` usa `std::fs::read_to_string` e la stessa funzione clipboard già implementata per la calcolatrice
- Stessa implementazione `gtk::PopoverMenu` usata per la lista normale — riutilizzare il pattern

**File da allegare:** `ui.rs`, `obsidian_bar.rs`, `list_model.rs`, `style.css` e gli altri file rilevanti.

---

## 📋 Prompt 5 — Context menu ricerca file

Devo aggiungere a Grunner un **menu contestuale (tasto destro) sui risultati della ricerca file** (modalità `:f`).

**Voci del menu:**
- **Apri** — stessa azione del doppio click / Enter
- **Copia percorso** — copia il path assoluto del file negli appunti
- **Copia contenuto** — visibile solo se il file è di testo (verificare con mime-type o estensione); legge il file e copia il testo negli appunti
- **Copia file** — copia il file negli appunti come oggetto (`GdkClipboard` con `GFile`), pronto per incollare con `Ctrl+V` in Nautilus o qualsiasi altra app che accetti file
- **Mostra nel file manager** — apre Nautilus nella cartella contenente il file

**Note implementative:**
- Il menu appare solo quando la modalità attiva è ricerca file (`:f`)
- Per "Copia file": usare `gdk::Clipboard` con content provider di tipo `GFile` — è il modo corretto su Wayland per copiare file, equivalente a "Copia" in Nautilus
- Per rilevare se un file è di testo: controllare il mime-type con `gio::content_type_guess` prima di mostrare la voce "Copia contenuto"
- Stessa implementazione `gtk::PopoverMenu` degli altri menu contestuali — riutilizzare il pattern

**File da allegare:** `ui.rs`, `list_model.rs`, `style.css` e gli altri file rilevanti per la gestione file e clipboard.

---

## 📋 Prompt 6 — Context menu modalità shell

Devo aggiungere a Grunner un **menu contestuale (tasto destro) sui risultati della modalità shell** (`:sh`).

**Voci del menu:**
- **Copia comando** — copia la stringa del comando negli appunti

**Note implementative:**
- Il menu appare solo quando la modalità attiva è shell (`:sh`)
- Stessa implementazione `gtk::PopoverMenu` degli altri menu contestuali — riutilizzare il pattern

**File da allegare:** `ui.rs`, `list_model.rs`, `style.css` e gli altri file rilevanti per la gestione comandi e config.

---

## 🗒️ Backlog (idee parcheggiate, non ancora in lavorazione)

- **Frequenza di lancio** — app usate più spesso salgono in cima alla lista
- **App recenti** — lista delle ultime N app lanciate quando l'entry è vuota
- **Alias** — soprannome personalizzato per un'app
- **Storico calcolatrice** — ultime N espressioni accessibili con `↑`
- **Tooltip titolo completo** — nella workspace bar, al passaggio del mouse
- **Animazione apertura finestra** — fade-in o slide-down
- **Shortcut `Alt+1…9` primi risultati lista** — lancia direttamente l'N-esimo risultato
- **Salva comando al volo** dalla modalità `:sh` — UX da definire
