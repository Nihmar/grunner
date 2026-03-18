# Analisi Architetturale del Codice Sorgente Grunner

**Data:** 18 Marzo 2026
**Scope:** Identificazione di problemi architetturali, accoppiamenti stretti, funzioni complesse e punti critici per le nuove feature.

---

## 1. Introduzione

Questo documento riporta i risultati dell'analisi statica del codice sorgente di Grunner. L'obiettivo è preparare la codebase per l'implementazione di nuove funzionalità (es. striscia preferiti, context menu) identificando e documentando deboli tecnici e opportunità di refactoring preventivo.

---

## 2. Analisi per File

### 2.1 `src/ui.rs`
**Responsabilità:** Costruzione completa dell'interfaccia GTK, gestione eventi, coordinamento flusso di ricerca.

**Problemi Identificati:**
1.  **Funzione Monolitica (`build_ui`):** La funzione principale conta oltre 500 righe. Gestisce CSS, tema, inizializzazione modello, creazione finestra, layout complesso (barre laterali hover), binding eventi e caricamento background.
2.  **Accoppiamento Stretto:** Mescola logica di presentazione (costruzione widget) con logica di business (inicializzazione modelli, polling thread).
3.  **Complessità Incapsulata:** La logica per la barra laterale (hover/reveal) è inline e difficile da riusare o testare isolatamente.

**Proposte Concrete:**
-   **Refactoring:** Suddividere `build_ui` in funzioni semantiche:
    -   `setup_environment()` (CSS, Tema, Global State)
    -   `create_main_window()`
    -   `build_layout()` (Struttura gerarchica widget)
    -   `connect_signals()` (Event handlers)
-   **Modularità:** Spostare `poll_apps` in un modulo dedicato (es. `background_loader.rs` o in `list_model`).

### 2.2 `src/list_model.rs`
**Responsabilità:** Gestione dati, logica di ricerca, aggiornamento store GTK, creazione factory UI.

**Problemi Identificati:**
1.  **God Class:** `AppListModel` gestisce ricerca app, obsidian, file, comandi shell, fornitori esterni e creazione factory UI.
2.  **Violazione Single Responsibility:** `create_factory` definisce la struttura HTML-like dei widget UI all'interno del modello dati.
3.  **Catena di IF complessa:** `bind_command_item` gestisce 5+ tipi di risultati diversi con una lunga catena `if/else`, violando il principio Open/Closed.

**Proposte Concrete:**
-   **Separazione Logica:** Introdurre un trait `SearchProvider` per disaccoppiare gli algoritmi di ricerca.
-   **Refactoring UI:** Spostare la logica di binding (`bind_*`) nel modulo `items` o in un helper UI, lasciando al model solo la gestione dei dati grezzi.

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

### 2.6 `src/search_provider.rs`
**Stato:** Ben encapsulato. Logica D-Bus complessa ma isolata.

---

## 3. Analisi trasversale: Accoppiamenti e Coesione

### 3.1 Accoppiamenti Stretti (Tight Coupling)
1.  **UI <-> Logica:** `ui.rs` dipende direttamente da `AppListModel` e `actions`. La dimensione di `build_ui` amplifica questo accoppiamento.
2.  **Config <-> Global State:** La creazione del config dipende da `global_state`.
3.  **Logica Ricerca <-> UI:** `AppListModel::create_factory` definisce la struttura UI. Il pattern attuale mescola Modello e Vista.

### 3.2 Mancanza di Interfacce Trait
I diversi "provider" di risultati (App, File, Obsidian, Calculator) sono implementati come rami condizionali in `AppListModel`.
**Proposta:** Introdurre un trait `SearchProvider`:
```rust
trait SearchProvider {
    fn search(&self, query: &str) -> Vec<ResultItem>;
    fn id(&self) -> &str;
}
```
Questo permetterebbe di aggiungere nuovi provider (es. Promemoria, Note rapide) senza modificare `AppListModel`.

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
-   **Posizione:** In `ui.rs`, tra barra ricerca e lista risultati.
-   **Impatto:** Modifica moderata a `build_ui`. Refactoring necessario prima di aggiungere complessità.
-   **Config:** Aggiungere `favorites: Vec<String>` in `Config`.

### 5.2 Context Menus (Prompt 2-6)
-   **Posizione:** In `list_model::create_factory`, aggiungendo `GtkGestureClick`.
-   **Impatto:** Modifica alla factory UI. Necessita accesso al model per azioni.

### 5.3 Estensibilità Config
-   Il sistema attuale richiede modifiche a 3 punti per aggiungere un campo. Valutare l'uso di `serde` + `toml` per deserializzazione automatica.

---

## 6. Interventi Consigliati (Prioritizzati)

### Alta Priorità
1.  **Refactoring di `ui::build_ui`**
    -   Suddividere in funzioni più piccole e semantiche.
    -   Obiettivo: Renderla gestibile prima di aggiungere la "Favorites Strip".

2.  **Introduzione Trait `SearchProvider`**
    -   Disaccoppiare la logica di ricerca dalle implementazioni concrete.
    -   Facilitare l'aggiunta di nuovi provider di risultati.

### Media Priorità
3.  **Pulizia `Config`**
    -   Rimuovere I/O da `Config::default()`.
    -   Garantire che i path vengano espansi solo al momento dell'uso.

4.  **Helper Icone**
    -   Estrazione della logica di selezione icona in `utils.rs`.

### Bassa Priorità
5.  **Gestione Errori Runtime**
    -   Propagare `Result` da `launch_app` fino alla UI per feedback utente.

---

## 7. Conclusione

L'analisi rivela una codebase funzionale ma con chiari segni di "god class" in `ui.rs` e `list_model.rs`. Il refactoring preventivo su `build_ui` e l'introduzione di un trait per i provider di ricerca sono fondamentali per supportare lo sviluppo futuro in modo sostenibile.
