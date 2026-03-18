# Analisi Architetturale del Codice Sorgente Grunner

**Data:** 18 Marzo 2026
**Ultimo Aggiornamento:** 18 Marzo 2026
**Scope:** Identificazione di problemi architetturali, accoppiamenti stretti, funzioni complesse e punti critici per le nuove feature.

---

## 1. Introduzione

Questo documento riporta i risultati dell'analisi statica del codice sorgente di Grunner. L'obiettivo è preparare la codebase per l'implementazione di nuove funzionalità (es. striscia preferiti, context menu) identificando e documentando deboli tecnici e opportunità di refactoring preventivo.

---

## 2. Analisi per File

### 2.1 `src/ui/window.rs`
**Responsabilità:** Costruzione completa dell'interfaccia GTK, gestione eventi, coordinamento flusso di ricerca.

**Problemi Identificati:**
1.  **Funzione Monolitica (Parzialmente Risolta):** La funzione `build_ui` è stata refattorizzata e ora delegate a funzioni helper (`setup_css`, `setup_model`, `create_window`, `build_main_layout`, `connect_*`, `start_background_loading`). Tuttavia, `build_main_layout` rimane lunga (~80 righe) e gestisce la costruzione complessa del layout.
2.  **Accoppiamento Stretto:** Mescola logica di presentazione (costruzione widget) con logica di business (inicializzazione modelli, polling thread) tramite chiamate dirette.
3.  **Complessità Incapsulata:** La logica per la barra laterale (hover/reveal) è inline in `build_sidebar` e difficile da riusare o testare isolatamente.

**Stato Attuale:**
-   Refactoring di `build_ui` completato (funzione principale < 100 righe).
-   `setup_css`, `setup_model`, `create_window`, `build_sidebar`, `build_main_layout`, `connect_window_signals`, `connect_search_signals`, `connect_list_signals`, `start_background_loading` sono funzioni separate.

**Proposte Concrete:**
-   **Ulteriore Refactoring:** Suddividere `build_main_layout` in costruttori più piccoli (es. `build_search_entry_area`, `build_results_list`).
-   **Modularità:** Spostare `poll_apps` in un modulo dedicato (es. `background_loader.rs` o in `list_model`).

### 2.2 `src/model/list_model.rs`
**Responsabilità:** Gestione dati, logica di ricerca, aggiornamento store GTK, creazione factory UI.

**Problemi Identificati:**
1.  **God Class:** `AppListModel` gestisce ricerca app, obsidian, file, comandi shell, fornitori esterni (D-Bus) e creazione factory UI.
2.  **Violazione Single Responsibility:** `create_factory` definisce la struttura UI e il binding dei dati all'interno del modello dati.
3.  **Catena di IF complessa:** `bind_command_item` gestisce 5+ tipi di risultati diversi con una lunga catena `if/else` (calcolo, script, obsidian grep, file path, output generico), violando il principio Open/Closed.
4.  **Accoppiamento Provider:** La gestione dei provider D-Bus è separata da quella dei provider locali (`AppProvider`, `CalculatorProvider`). I provider D-Bus sono gestiti tramite canali MPSC e polling asincrono, mentre i provider locali sono chiamati sincronamente.

**Stato Attuale:**
-   Il trait `SearchProvider` esiste in `src/providers/mod.rs` ed è implementato da `AppProvider` e `CalculatorProvider`.
-   I fornitori D-Bus (`dbus_provider`) non implementano `SearchProvider` ma sono gestiti internamente da `AppListModel` tramite `run_provider_search`.

**Proposte Concrete:**
-   **Refactoring UI:** Spostare la logica di binding (`bind_*`) nel modulo `items` o in un helper UI, lasciando al model solo la gestione dei dati grezzi. **[COMPLETATO]** Creato `src/ui/list_factory.rs` per gestire la creazione della factory e il binding dei dati.
-   **Unificazione Provider:** Valutare se creare un trait async o una wrapper struct per unificare la gestione di provider locali e D-Bus, riducendo la complessità in `AppListModel`.

**Stato Attuale:**
-   **Factory UI Separata:** La logica di creazione della factory e binding dei dati è stata spostata in `src/ui/list_factory.rs`.
-   **ActiveMode Condiviso:** L'enum `ActiveMode` è stato spostato in `src/app_mode.rs` per essere accessibile sia al modello che alla UI.
-   **Riduzione Complessità:** `AppListModel::create_factory` è ora una semplice delega a `list_factory::create_factory`.
-   **Layout UI Corretto:** Ripristinato il layout originale con icona + (nome + descrizione) su due righe.
-   **Logica di Binding Ripristinata:** Tutte le funzionalità originali sono state ripristinate:
    - Icone basate sul content type per i risultati di `:obg` (Obsidian Grep)
    - Icone markdown (`text-markdown`) per i risultati di `:ob` (Obsidian File)
    - Path accorciati con tilde (`~`) per i file sotto la home directory
    - Path relativi al vault per i risultati Obsidian
    - Icone generiche basate sul content type per file non-Obsidian
    - Gestione corretta della visibility della descrizione con `set_desc`

### 2.3 `src/config.rs`
**Responsabilità:** Parsing e caricamento configurazione TOML.

**Problemi Identificati:**
1.  **I/O Implicito:** `Config::default()` chiama `expand_home` che accede a `global_state::get_home_dir`. Questo rende la creazione del config dipendente da stato globale e difficile da testare.
2.  **Hardcoding:** I valori di default sono replicati in `default()`, `default_toml()` e costanti.

**Proposte Concrete:**
-   Separare la costruzione del config dalla risoluzione dei path assoluti.
-   Garantire che `Config` possa essere istanziato senza effetti collaterali (I/O).

### 2.4 `src/global_state.rs`
**Responsabilità:** Stato globale thread-local (Tokio Runtime, Reloaders).

**Problemi Identificati:**
1.  **Accesso Implicito:** Rende le dipendenze opache e i test più complessi.
2.  **Runtime Permanente:** Il Tokio Runtime viene inizializzato all'avvio e non viene mai distrutto.

**Proposte Concrete:**
-   Valutare l'iniezione delle dipendenze (es. passare `RuntimeHandle` esplicitamente) dove possibile, mantenendo `global_state` per i reloaders se necessario.

### 2.5 `src/items/mod.rs` & `src/launcher.rs`
**Stato:** Buona coesione. `launcher.rs` gestisce efficacemente caching e parsing .desktop.

### 2.6 `src/providers/dbus_provider.rs`
**Stato:** Ben encapsulato. Logica D-Bus complessa ma isolata.
**Nota:** Questo modulo fornisce la scoperta dei provider e l'esecuzione delle query asincrone, ma non implementa il trait `SearchProvider` definito in `src/providers/mod.rs`. È gestito separatamente da `AppListModel`.

---

## 3. Analisi trasversale: Accoppiamenti e Coesione

### 3.1 Accoppiamenti Stretti (Tight Coupling)
1.  **UI <-> Logica:** `ui.rs` dipende direttamente da `AppListModel` e `actions`. La dimensione di `build_ui` amplifica questo accoppiamento.
2.  **Config <-> Global State:** La creazione del config dipende da `global_state`.
3.  **Logica Ricerca <-> UI:** `AppListModel::create_factory` definisce la struttura UI. Il pattern attuale mescola Modello e Vista.

### 3.2 Interfacce Trait (Esistenti e Mancanti)
I diversi "provider" di risultati sono gestiti in modo eterogeneo:
1.  **Provider Locali (`AppProvider`, `CalculatorProvider`):** Implementano il trait `SearchProvider` in `src/providers/mod.rs`. Sono disaccoppiati da `AppListModel`.
2.  **Provider D-Bus (`dbus_provider`):** Non implementano `SearchProvider`. Sono gestiti tramite logica specifica asincrona (`run_search_streaming`) all'interno di `AppListModel`.
3.  **Provider Interni (File, Obsidian, Shell):** Sono implementati come metodi diretti in `AppListModel` (`run_file_search`, `handle_obsidian`, `handle_sh`).

**Proposta:**
-   **Unificazione (Opzionale/Complesso):** Creare un trait async o una wrapper struct per unificare provider locali e D-Bus.
-   **Estensibilità:** Per nuovi provider sincroni, usare `SearchProvider`. Per provider asincroni, valutare un pattern simile a D-Bus (canali MPSC) ma generico.

### 3.3 Duplicazioni di Codice
-   **Icone:** Logica simile per trovare iconi valide in `list_model.rs` e `power_bar.rs`. Da estrarre in `utils.rs`.
-   **Gestione Path:** `expand_home` è usato in modo inconsistente tra `Config::default` e runtime.

---

## 4. Analisi della Complessità e Flussi Critici

### 4.1 Funzioni Troppo Lunghe
-   **`ui::build_ui` (534 righe):** Candidata principale per refactoring.
-   **`list_model::bind_command_item`:** Catena di `if/else` per 5+ tipi di risultati. Suggerito pattern Strategy.
-   **`search_provider::parse_icon_variant`:** Complessa ma giustificata dalla specifica D-Bus.

### 4.2 Flusso di Inizializzazione
1.  `main()` -> `logging::init()`
2.  `config::load()` (I/O sincrono)
3.  `app.connect_activate` -> `ui::build_ui` (caricamento pesante sincrono)
4.  Background thread: `launcher::load_apps`
5.  Idle callback: `poll_apps` aggiorna UI

**Nota:** Il caricamento config è sincrono. Se il file è su filesystem lento, l'avvio è ritardato.

---

## 5. Punti Critici per Nuove Feature

### 5.1 Striscia Preferiti (Prompt 1)
-   **Posizione:** In `src/ui/window.rs` (o modulo UI dedicato), tra barra ricerca e lista risultati.
-   **Impatto:** Modifica a `build_main_layout`. Il refactoring già completato facilita l'inserimento del nuovo widget.
-   **Config:** Aggiungere `favorites: Vec<String>` in `Config` (in `src/core/config.rs`).
-   **Dati:** Potrebbe richiedere una nuova struttura dati in `src/model/items/` per rappresentare un preferito.

### 5.2 Context Menus (Prompt 2-6)
-   **Posizione:** In `src/model/list_model.rs` nella funzione `create_factory`, aggiungendo `GtkGestureClick` o `GtkPopover` ai list items.
-   **Impatto:** Modifica alla factory UI e alla logica di binding. Necessita accesso al model per azioni (es. "Apri percorso", "Copia").
-   **Integrazione:** La logica di attivazione (`item_activation.rs`) dovrà espandersi per gestire azioni contestuali.

### 5.3 Estensibilità Config
-   Il sistema attuale richiede modifiche a 3 punti per aggiungere un campo. Valutare l'uso di `serde` + `toml` per deserializzazione automatica.

---

## 6. Interventi Consigliati (Prioritizzati)

### Alta Priorità
1.  **[X] Refactoring di `ui::build_ui`**
    -   Suddividere in funzioni più piccole e semantiche.
    -   Obiettivo: Renderla gestibile prima di aggiungere la "Favorites Strip".
    -   **Stato:** Completato. La funzione è stata suddivisa in `setup_css`, `setup_model`, `create_window`, `build_sidebar`, `build_main_layout`, `connect_window_signals`, `connect_search_signals`, `connect_list_signals` e `start_background_loading`.
    -   **Nota:** `build_main_layout` potrebbe essere ulteriormente spezzata.

2.  **[X] Introduzione Trait `SearchProvider`**
    -   Disaccoppiare la logica di ricerca dalle implementazioni concrete.
    -   Facilitare l'aggiunta di nuovi provider di risultati.
    -   **Stato:** Completato per i provider locali. Introdotto trait `SearchProvider` con implementazioni per `AppProvider` e `CalculatorProvider`.
    -   **Nota:** I fornitori D-Bus non implementano questo trait e richiedono gestione separata (async).

3.  **[X] Riordino struttura moduli**
    -   Raggruppare i file in sottocartelle (`core`, `model`, `providers`, `ui`).
    -   **Stato:** Completato. I file sono stati organizzati in una struttura logica che migliora la manutenibilità.

### Media Priorità
4.  **[X] Pulizia `Config`**
    -   Rimuovere I/O da `Config::default()`.
    -   Garantire che i path vengano espansi solo al momento dell'uso.
    -   **Stato:** Completato. `app_dirs` ora memorizza raw paths (`Vec<String>`) invece di expanded paths (`Vec<PathBuf>`). Aggiunto metodo `expanded_app_dirs()` per l'espansione lazy. I path vengono espansi solo quando necessario.

5.  **[X] Helper Icone**
    -   Estrazione della logica di selezione icona in `utils.rs`.
    -   **Stato:** Completato. Creata funzione `get_file_icon(file_path: &str) -> gio::Icon` in `src/utils.rs`. La logica duplicata in `list_factory.rs` ora usa questo helper.

### Bassa Priorità
6.  **[X] Gestione Errori Runtime**
    -   Propagare `Result` da `launch_app` fino alla UI per feedback utente.
    -   **Stato:** Completato. Aggiunta funzione `show_error_notification()` in `src/actions.rs` che mostra una notifica GTK quando il lancio di un'app fallisce.

---

## 7. Aggiornamenti Recenti (2026)

### 7.1 Fix Icone Modalità Grep (`:obg`, `:fg`)

**Problema Identificato:**
- In modalità `:obg` e `:fg`, le icone venivano mostrate come file generici (bianche) invece che con l'icona corretta per il tipo di file (es. markdown per file `.md`).
- Questo era dovuto al fatto che l'output grep ha formato `path:line:content`, e il codice passava l'intera stringa a `content_type_guess()` invece di estrarre solo il path del file.

**Fix Applicato (18 Marzo 2026):**
- In `src/ui/list_factory.rs`, la funzione `bind_command_item` ora rileva correttamente i risultati grep controllando se la linea:
  1. Ha `mode == ActiveMode::ObsidianGrep`
  2. Oppure contiene `:` e non inizia con `/`
  3. Oppure inizia con `/` e contiene almeno 2 `:`
- Viene ora estratto solo il path del file prima di chiamare `content_type_guess()`, permettendo a GTK di determinare l'icona corretta.

**Stato:** ✅ Risolto

### 7.2 Fix Icona Markdown per `:ob`

**Problema Identificato:**
- L'icona `text-x-markdown` non esiste nel tema GTK (Adwaita). L'icona corretta è `text-markdown`.

**Fix Applicato (18 Marzo 2026):**
- Aggiornato `list_factory.rs` per usare `text-markdown` invece di `text-x-markdown` in:
  - Linea 239: fallback per risultati grep non parsabili
  - Linea 251: modalità ObsidianFile (`:ob`)

**Stato:** ✅ Risolto

---

## 8. Conclusione

L'analisi rivela una codebase funzionale ma con chiari segni di "god class" in `src/model/list_model.rs` e `src/ui/window.rs` (nonostante il refactoring di `build_ui`). La separazione tra provider locali (trait `SearchProvider`) e asincroni (D-Bus) è una debolezza architetturale che complica l'estensione.

Le modifiche strutturali già effettuate (refactoring di `build_ui`, introduzione di `SearchProvider`) posizionano bene il progetto per l'implementazione delle nuove feature (Favorites Strip, Context Menu). I prossimi passi dovrebbero concentrarsi sulla riduzione della complessità di `AppListModel` e sull'unificazione della gestione dei provider.

**Nota:** Il sistema di rilevamento icone è stato recentemente corretto per le modalità grep. Le attività di Media e Bassa priorità (Pulizia Config, Helper Icone, Gestione Errori Runtime) sono state completate. Nuovi test manuali sono consigliati per verificare il comportamento in tutte le combinazioni di modalità.
