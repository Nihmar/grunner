# Analisi Architetturale del Codice Sorgente Grunner

**Data:** 18 Marzo 2026  
**Ultimo Aggiornamento:** 18 Marzo 2026  
**Scope:** Identificazione problemi architetturali, accoppiamenti stretti, funzioni complesse e punti critici per nuove feature.

---

## Sommario

| File | Righe | Problemi Principali |
|------|-------|-------------------|
| `model/list_model.rs` | 959 | **God class** - gestisce ricerca, modello, threading, polling, factory |
| `providers/dbus_provider.rs` | 848 | Provider complesso con parsing D-Bus intricato |
| `actions.rs` | 639 | Azioni eterogenee (launch, power, file, Obsidian, settings) |
| `ui/window.rs` | 590 | UI principale, molti segnali e controller |
| `workspace_bar.rs` | 509 | Logica D-Bus per workspace bar |
| `calculator.rs` | 514 | Modulo ben isolato, buona coesione |
| `launcher.rs` | 480 | Scanning .desktop file con caching binario |

---

## 1. Accoppiamenti Stretti (Tight Coupling)

### 1.1 Stato Globale Condiviso (`global_state.rs`)

**Problema:**  
Il modulo `global_state.rs` usa `OnceLock` per memorizzare:
- `HOME_DIR` - home directory
- `TOKIO_RUNTIME` - runtime async
- `CONFIG_RELOADER` - callback per hot-reload configurazione
- `THEME_RELOADER` - callback per hot-reload tema

Questo rende le dipendenze **opache** e i **test impossibili** senza mock.

```rust
// global_state.rs
pub static HOME_DIR: OnceLock<String> = OnceLock::new();
pub static TOKIO_RUNTIME: OnceLock<Runtime> = OnceLock::new();
pub static CONFIG_RELOADER: OnceLock<Box<dyn Fn(&Config)>> = OnceLock::new();
pub static THEME_RELOADER: OnceLock<Box<dyn Fn(&Config, &Display)>> = OnceLock::new();
```

### 1.2 UI Accoppiata al Modello (`ui/window.rs` ↔ `model/list_model.rs`)

**Problema:**  
`window.rs` crea direttamente `AppListModel` e connette segnali. Questo tight coupling rende difficile:
- Testare la UI indipendentemente
- Riutilizzare `AppListModel` in contesti diversi
- Integrare il modello in altre applicazioni

### 1.3 Azioni Accoppiate a Modello (`item_activation.rs`)

**Problema:**  
`activate_item()` riceve `&AppListModel` per accedere a `obsidian_cfg`. Le azioni dovrebbero ricevere solo i **dati necessari**, non l'intero modello.

```rust
pub fn activate_item(obj: &glib::Object, model: &AppListModel, mode: AppMode, timestamp: u32) {
    if let Some(cfg) = &model.obsidian_cfg {  // Solo per Obsidian
        perform_obsidian_action(...);
    }
}
```

---

## 2. Funzioni Troppo Lunghe o Complesse

### 2.1 `AppListModel` - God Class (959 righe)

**Responsabilità Multiple:**
- Gestione store GTK (`store`, `selection`)
- Ricerca fuzzy (`all_apps`, `providers`)
- Esecuzione comandi (`:ob`, `:obg`, `:f`, `:fg`, `:sh`)
- Threading (`task_gen`, `bump_task_gen`)
- Polling asincrono (`SubprocessPoller`, `ProviderSearchPoller`)
- Factory UI delegation
- Config hot-reload

**Proposta:** Suddividere in:
- `SearchCoordinator` - orchestrazione ricerca
- `CommandHandler` - astrazione per comandi colon
- `ObsidianSearcher`, `FileSearcher`, `ShellHandler` - handler specifici
- `PollingManager` - gestione unified polling

### 2.2 `dbus_provider.rs` - Parsing Complesso (848 righe)

**Problemi:**
- Parsing Icon D-Bus intricato (80+ righe di match annidati)
- Streaming con canali MPSC per risultati batch

**Proposta:** Estrarre `IconParser` in modulo separato

### 2.3 `actions.rs` - Module Conglomerato (639 righe)

**6 Responsabilità:**
- `launch_app()` - launching applicazioni
- `power_action()` - logout, suspend, reboot, poweroff
- `open_file_or_line()` - apertura file
- `perform_obsidian_action()` - azioni Obsidian
- `open_settings()` - apertura settings
- `which()` - ricerca executables

**Proposta:** Directory `actions/` con sottomoduli

---

## 3. Interfacce Trait e Provider

### 3.1 Sistema Provider Attuale

**Esistente:**
```rust
pub trait SearchProvider {
    fn search(&self, query: &str) -> Vec<glib::Object>;
}
```

**Implementato da:**
- `AppProvider` - ricerca applicazioni
- `CalculatorProvider` - calcolatrice

**Non Implementano il Trait:**
- `DbusSearchProvider` - gestito separatamente per async
- `ObsidianSearcher` - metodo diretto in `AppListModel`
- `FileSearcher` - metodo diretto in `AppListModel`
- `ShellHandler` - inline in `AppListModel`

### 3.2 Proposta: Trait `CommandProvider`

```rust
pub trait CommandProvider {
    fn mode(&self) -> &str;  // ":ob", ":fg", etc.
    fn search(&self, arg: &str) -> Vec<CommandItem>;
    fn validate(&self, cfg: &Config) -> Result<(), ConfigError>;
}
```

---

## 4. Duplicazioni di Codice

### 4.1 Esecuzione Subprocess

**Duplicazione:** Thread spawn + channel + polling ripetuto in `run_subprocess()`, `run_provider_search()`

**Proposta:** Creare `SubprocessRunner` con callback per risultati

### 4.2 Path Handling

```rust
pub fn expand_home(path: &str) -> PathBuf { ... }
pub fn contract_home(path: &Path) -> String { ... }
fn config_path() -> PathBuf { ... }
fn cache_path() -> PathBuf { ... }
```

**Proposta:** Unificare in `PathUtils` trait

---

## 5. Punti Critici per Feature Future

### 5.1 Striscia Preferiti

**Posizione:** `ui/favorites_bar.rs` - tra search entry e results list

**Impatto su `window.rs`:**
- Estrarre `search_entry_area` come funzione separata
- Permettere inserimento widget tra entry e results

### 5.2 Context Menu

**Posizione:** `list_factory.rs` - `bind_*` funzioni + `item_activation.rs`

**Modifiche Necessarie:**
- Widget devono avere `EventController` per click destro
- `activate_item()` deve gestire azioni menu contestuali

### 5.3 Estensibilità Config

**Problema:** Per aggiungere un campo servono modifiche in 6+ posizioni

**Proposta:** Usare serde con deserializzazione diretta

---

## 6. Interventi Prioritizzati

### Alta Priorità

| # | Intervento | File | Impatto Feature |
|---|------------|------|-----------------|
| 1 | Estrarre `CommandHandler` da `AppListModel` | list_model.rs | Favorites, Context Menu |
| 2 | Refactoring `actions.rs` in sottomoduli | actions.rs | Context Menu |
| 3 | Creare trait `CommandProvider` | providers/ | Favorites, Context Menu |

### Media Priorità

| # | Intervento | File | Impatto Feature |
|---|------------|------|-----------------|
| 4 | Pattern Strategy per `bind_command_item()` | list_factory.rs | Context Menu |
| 5 | Unificare subprocess execution | list_model.rs | Debug, Test |
| 6 | Helper path in utils.rs | utils.rs | Manutenzione |

### Bassa Priorità

| # | Intervento | File | Impatto Feature |
|---|------------|------|-----------------|
| 7 | Aggiungere trait `Activatable` | actions.rs | Test, Estensibilità |
| 8 | Mock `global_state` per test | global_state.rs | Test Coverage |
| 9 | Refactoring icon parsing D-Bus | dbus_provider.rs | Manutenzione |

---

## 7. Struttura Moduli Proposta

```
src/
├── actions/                    # NUOVO: Modulo azioni refattorizzato
│   ├── mod.rs
│   ├── app_launcher.rs         # launch_app, terminal discovery
│   ├── power.rs                # power_action
│   ├── file_opener.rs          # open_file_or_line
│   └── notification.rs          # show_error_notification
├── commands/                   # NUOVO: Gestione comandi colon
│   ├── mod.rs
│   ├── command_provider.rs     # Trait CommandProvider
│   ├── obsidian.rs             # :ob, :obg handler
│   ├── file_search.rs          # :f, :fg handler
│   └── shell.rs                # :sh handler
├── providers/
│   ├── mod.rs
│   ├── app.rs                  # AppProvider
│   ├── calculator.rs            # CalculatorProvider
│   ├── dbus_provider.rs        # D-Bus integration
│   └── command.rs              # CommandProvider implementations
├── core/
│   ├── config.rs
│   ├── global_state.rs         # TODO: iniettare per test
│   └── theme.rs
├── model/
│   ├── list_model.rs           # TODO: estrarre SearchCoordinator
│   └── items/
├── ui/
│   ├── window.rs               # TODO: Widget composition
│   ├── list_factory.rs         # TODO: Strategy pattern
│   ├── favorites_bar.rs         # NUOVO: Feature Favorites
│   └── context_menu.rs         # NUOVO: Feature Context Menu
└── utils.rs
```

---

## 8. Conclusioni e Raccomandazioni

### Punti di Forza
1. **Modularità base** - struttura in `core/`, `model/`, `providers/`, `ui/`
2. **Test coverage** - buona copertura per calculator, config, app_mode
3. **Documentazione** - Rustdoc estensivo
4. **Separazione UI/Model** parziale - `list_factory.rs` separato

### Criticità Principali
1. **God class `AppListModel`** - 959 righe con troppe responsabilità
2. **Stato globale opaco** - `global_state.rs` difficile da testare
3. **Azioni conglomerate** - `actions.rs` con 6 responsabilità
4. **Provider eterogenei** - non tutti implementano `SearchProvider`

### Impatto Feature Future
- **Favorites Strip**: Impatto **basso** - richiede solo aggiunta widget
- **Context Menu**: Impatto **medio-alto** - richiede refactoring `actions.rs` e `item_activation.rs`

### Raccomandazione Finale
Prima di implementare Context Menu, completare il refactoring di `actions.rs` per evitare ulteriore accrescimento della complessità. Il refactoring di `AppListModel` può essere fatto incrementalmente durante l'implementazione delle nuove feature.

---

## 9. Aggiornamenti Recenti (18 Marzo 2026)

### Fix Completati

1. **Fix Icone Modalità Grep (`:obg`, `:fg`)**  
   Output grep ora parsato correttamente per determinare tipo file

2. **Fix Icona Markdown per `:ob`**  
   Usato `text-markdown` invece di `text-x-markdown` inesistente

3. **Pulizia Config**  
   `app_dirs` ora memorizza raw paths, espansione lazy con `expanded_app_dirs()`

4. **Helper Icone**  
   `get_file_icon()` estratta in `utils.rs`

5. **Gestione Errori Runtime**  
   `show_error_notification()` aggiunta per feedback utente su fallimenti launch
