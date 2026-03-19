# Grunner Codebase Audit Report

> **Data:** 19 marzo 2026  
> **Versione Grunner:** 2.1.0

---

## Sommario Esecutivo

La codebase di Grunner è **strutturalmente solida** ma presenta aree di miglioramento concentrate in:

1. **Duplicazione codice** (~400+ linee duplicate identificabili)
2. **File troppo lunghi** (`dbus_provider.rs` 848 linee, `workspace_bar.rs` 654 linee)
3. **Accoppiamento moderato** (`AppListModel` con troppe responsabilità)

Il sistema è pronto per le nuove feature ma richiede refactoring preventivo per i context menu.

---

## Struttura Attuale

```
src/
├── main.rs              (177 linee)
├── lib.rs               (31 linee)
├── app_mode.rs          (158 linee)
├── calculator.rs
├── command_handler.rs   (229 linee)
├── item_activation.rs   (229 linee)
├── launcher.rs          (490 linee)
├── utils.rs             (151 linee)
├── core/
│   ├── config.rs        (664 linee)
│   ├── global_state.rs  (96 linee)
│   └── theme.rs
├── model/
│   ├── list_model.rs    (818 linee)
│   └── items/           (5 file GObject)
├── providers/
│   ├── mod.rs           (175 linee)
│   └── dbus_provider.rs (848 linee)
├── ui/
│   ├── window.rs        (1316 linee)
│   ├── list_factory.rs  (438 linee)
│   ├── pinned_strip.rs  (264 linee)
│   ├── power_bar.rs     (212 linee)
│   ├── obsidian_bar.rs  (108 linee)
│   ├── workspace_bar.rs (654 linee)
│   └── style.css
├── actions/
│   ├── mod.rs           (57 linee)
│   ├── launcher.rs      (143 linee)
│   ├── power.rs         (114 linee)
│   ├── obsidian.rs      (250 linee)
│   ├── file.rs          (86 linee)
│   └── settings.rs
└── settings_window/
    ├── mod.rs           (156 linee)
    └── tabs/            (6 file tab)
```

---

## Analisi File-by-File

---

### 1. `ui/window.rs` (1316 linee)

**Responsabilità attuali:**

- Costruzione UI principale (`build_ui`)
- Context menu per tutti i modi (`build_normal_context_menu`, `build_obsidian_context_menu`, `build_file_search_context_menu`, `build_shell_context_menu`)
- Keyboard controller
- Background app loading
- Clipboard operations (duplicated)

**Problemi identificati:**

1. **Duplicazione massiccia nei context menu** - Le 4 funzioni `build_*_context_menu` hanno ~80% di codice duplicato:
   - Creazione popover
   - Pattern `WeakRef<Popover>` per chiusura
   - `make_menu_button` con CSS identico
   - Logica di attivazione item simile

2. **Too many arguments antipattern** - `setup_list_context_menu` ha 10 parametri, `update_pinned_strip` ha 9, `start_background_loading` ha 9

3. **Duplicazione clipboard** - `copy_text_to_clipboard` e `copy_file_to_clipboard` esistono sia qui che in `actions/file.rs`

4. **Helper functions dispersi** - `is_text_file`, `open_in_file_manager`, `open_with_default_app` sono in fondo al file ma potrebbero essere moduli condivisi

5. **`build_ui` lunga ma organizzata** - In realtà ben strutturata (~120 linee) con helper functions ben separati. Non è il problema principale.

**Proposte concrete:**

- Estrarre context menu in `ui/context_menu.rs` con un `ContextMenuBuilder` trait
- Creare `ui/clipboard.rs` centralizzato
- Wrappare i parametri in struct (`ContextMenuParams`, `LoadingParams`)

---

### 2. `model/list_model.rs` (818 linee)

**Responsabilità attuali:**

- Gestione store GTK
- Debounce logic (search + command)
- Provider coordination
- Subprocess execution
- Obsidian/file search via subprocess

**Problemi identificati:**

1. **Troppe responsabilità in una struct** - `AppListModel` gestisce:
   - App list
   - Commands list
   - All 4 debounce timers (`command_debounce`, `search_debounce`, e relativi source IDs)
   - Search providers
   - Active mode
   - Generation tracking per cancellazione task

2. **Pattern misto async** - Usa sia `std::thread::spawn` + channel + `glib::idle_add_local_once` che tokio runtime - funziona ma è confusionario

3. **Search logic hardcoded** - `run_file_search`, `run_file_grep` sono comandi shell hardcoded, non estendibili

4. **`SubprocessRunner` e `ProviderSearchPoller` sono buoni pattern** - Andrebbero estratti in moduli separati

**Proposte concrete:**

- Estrarre `SubprocessRunner` in `model/runner.rs`
- Estrarre `ProviderSearchPoller` in `model/provider_poller.rs`
- Creare un `SearchBackend` trait per astrazione file search
- Considerare uno state machine pattern per i debounce

---

### 3. `providers/mod.rs` (175 linee)

**Responsabilità attuali:**

- `SearchProvider` trait (sync)
- `CommandProvider` trait
- `AppProvider` implementation
- `CalculatorProvider` implementation

**Problemi identificati:**

1. **Due trait separati** - `SearchProvider` e `CommandProvider` potrebbero essere unificati o avere un trait base comune

2. **`AppProvider` non è estendibile** - Il `fuzzy_matcher` è interno, non configurabile

**Proposte concrete:**

- Unificare in `SearchProvider` con metodo `get_results()`
- Aggiungere `priority()` ai provider per ordinamento
- Esporre `AppProvider::set_max_results()` pubblico

---

### 4. `providers/dbus_provider.rs` (848 linee)

**Responsabilità attuali:**

- Provider discovery da .ini files
- Async D-Bus queries
- Icon parsing (molto complesso)
- Result activation

**Problemi identificati:**

1. **File troppo lungo** - 848 linee per un singolo file, dovrebbe essere un modulo

2. **Icon parsing iper-complesso** - `parse_icon_variant`, `extract_themed`, `extract_file` sono ~200 linee di pattern matching su D-Bus variants

3. **Duplicazione path search** - `resolve_app_icon` e `resolve_from_desktop` in `workspace_bar.rs` fanno la stessa cosa

4. **Runtime management duplicato** - Ha `get_runtime()` che chiama `global_state::get_tokio_runtime()`

**Proposte concrete:**

- Split in `providers/dbus/` con `discovery.rs`, `query.rs`, `icons.rs`
- Estrarre utility `resolve_desktop_icon` in `utils.rs`
- Documentare meglio i formati D-Bus attesi

---

### 5. `ui/workspace_bar.rs` (654 linee)

**Responsabilità attuali:**

- D-Bus communication con window-calls extension
- Window fetching e filtering
- Icon resolution
- UI rendering

**Problemi identificati:**

1. **File lungo** - 654 linee, logica D-Bus mescolata con UI

2. **Duplicazione icon resolution** - `resolve_from_desktop` (linee 244-280) è identico a `resolve_app_icon` in `dbus_provider.rs`

3. **Runtime management locale** - Ha il suo `TOKIO_RT` invece di usare `global_state`

4. **`resolve_icon` molto complessa** - 40+ linee per trovare un'icona, potrebbe essere semplificata

**Proposte concrete:**

- Estrarre `resolve_desktop_icon` in `utils.rs` condivisa
- Split `providers/dbus/` per le parti di icon resolution
- Considerare un `WorkspaceBarState` struct

---

### 6. `core/global_state.rs` (96 linee)

**Responsabilità attuali:**

- HOME directory caching
- Tokio runtime singleton
- Config hot-reload callbacks
- Theme hot-reload callbacks

**Problemi identificati:**

1. **Thread-local per reloader** - Pattern insolito, potrebbe essere più chiaro con un trait `Reloadable`

2. **Due runtime tokio** - `workspace_bar.rs` ne crea uno own, potenzialmente conflittuale

3. **No cleanup** - I OnceLock non vengono mai rilasciati

**Proposte concrete:**

- Unificare tutti i tokio runtime in `global_state`
- Creare trait `HotReloadable` per config/theme
- Considerare `AppState` struct invece di global dispersi

---

### 7. `command_handler.rs` (229 linee)

**Responsabilità attuali:**

- Parsing comandi colon (`:ob`, `:f`, `:fg`, `:sh`)
- Routing a handler appropriati
- Obsidian/file/custom script execution

**Problemi identificati:**

1. **Hardcoded command set** - Aggiungere nuovi comandi richiede modifiche a questo file + enum

2. **Accesso diretto a internals** - Accede `self.model.store`, `self.model.task_gen` direttamente

3. **`validated_vault_path` duplicato** - Altrove c'è logica simile

**Proposte concrete:**

- Creare trait `CommandHandler` con `handle(query) -> Results`
- Estrarre validazioni in `utils.rs`
- Aggiungere plugin system per comandi custom

---

### 8. `actions/` module

**Responsabilità attuali:**

- App launching (`launcher.rs`)
- Power actions (`power.rs`)
- Obsidian operations (`obsidian.rs`)
- File operations (`file.rs`)

**Problemi identificati:**

1. **`file.rs` ha duplicazione clipboard** - `open_file_or_line` chiama clipboard ma `window.rs` ha funzioni simili

2. **`launcher.rs::find_terminal_impl` hardcoded** - Lista terminali hardcoded, non configurabile

3. **`open_uri` in `mod.rs` duplica funzionalità** - `open_with_default_app` in `window.rs` fa la stessa cosa

**Proposte concrete:**

- Unificare clipboard operations in `utils/clipboard.rs`
- Rendere `terminal_candidates` configurabile
- Merge `open_uri` e `open_with_default_app`

---

## Duplicazioni Identificate

| Pattern | Locations |
|---------|----------|
| Clipboard text | `window.rs:983-988`, `actions/file.rs:81-83` |
| Clipboard file | `window.rs:990-998` |
| Desktop icon resolve | `dbus_provider.rs:295-358`, `workspace_bar.rs:244-280` |
| Open with xdg-open | `window.rs:1022-1026`, `actions/mod.rs:45-57` |
| make_menu_button | `window.rs:974-981` |
| make_icon_button | `power_bar.rs:32-57` |

---

## Punti Critici per Feature Future

### Favoriti/Pinned Strip (Prompt 1)
**Invasività: BASSA**

Il sistema pinned è già implementato in `ui/pinned_strip.rs` e `pinned_strip` in `window.rs`. Per aggiungere la strip preferiti:

- La struttura esiste già: `Rc<RefCell<Vec<String>>>` per desktop IDs
- Il CSS class management è in `style.css`
- I context menu per aggiungere/rimuovere sono in `build_normal_context_menu`

**Refactoring preventivo suggerito:**

- Estrarre `build_favorites_strip` da `build_pinned_strip` (sono lo stesso concetto!)
- Unificare `pinned_strip` e `favorites_strip` se possibile

### Context Menu (Prompt 2-6)
**Invasività: ALTA se non si refattorizza prima**

I 4 context menu (`build_normal_context_menu`, `build_obsidian_context_menu`, `build_file_search_context_menu`, `build_shell_context_menu`) sono ~300 linee totali con 80% duplicazione.

**Refactoring preventivo OBBLIGATORIO:**

1. Creare `ui/context_menu.rs` con:

```rust
pub trait ContextMenuBuilder {
    fn build_menu(&self, obj: &Object, ctx: &MenuContext) -> Popover;
    fn menu_items(&self) -> Vec<MenuItem>;
}
```

2. Implementare `NormalMenuBuilder`, `ObsidianMenuBuilder`, etc.

3. Estrarre `MenuItem` struct con label, action, icon

### Sistema Config Estensibilità
**Stato: BUONO ma migliorabile**

`Config` struct è pulito, ma:

- Nuovi commandi richiedono modifica a `CommandConfig` + `command_handler.rs`
- Nuovi provider richiedono modifica a `AppListModel::new`

**Suggerimento:**

- Aggiungere `serde(flatten)` per config plugins
- Creare `trait ConfigurableProvider`

---

## Lista Prioritizzata Interventi

### 🔴 ALTA PRIORITÀ (prima delle feature)

#### 1. Estrarre context menu system (`ui/context_menu.rs`)

- **Problema:** ~200 linee duplicate nei 4 context menu
- **Impatto:** Elimina duplicazione, necessario per i Prompt 2-6
- **Stima:** 2-3 ore
- **Deliverable:**
  - `MenuItem` struct con `{label, action, icon, enabled}`
  - `MenuContext` struct con `{model, window, entry, ...}`
  - `ContextMenuBuilder` trait
  - Implementazioni: `NormalMenuBuilder`, `ObsidianMenuBuilder`, `FileSearchMenuBuilder`, `ShellMenuBuilder`
  - Migrazione di `build_normal_context_menu`, etc. alle nuove implementazioni

#### 2. Unificare clipboard utilities (`utils/clipboard.rs`)

- **Problema:** Duplicato in `window.rs` e `actions/file.rs`
- **Impatto:** Elimina duplicazioni in 3+ file
- **Stima:** 30 minuti
- **Deliverable:**
  ```rust
  pub fn copy_text(text: &str)
  pub fn copy_file(path: &str) -> Result<()>
  pub fn copy_content(path: &str) -> Result<()>
  ```

#### 3. Unificare desktop icon resolution (`utils/desktop.rs`)

- **Problema:** `resolve_app_icon` in `dbus_provider.rs` e `resolve_from_desktop` in `workspace_bar.rs` fanno la stessa cosa
- **Impatto:** Elimina ~80 linee duplicate
- **Stima:** 1 ora
- **Deliverable:**
  ```rust
  pub fn resolve_desktop_icon(wm_class: &str) -> Option<(name: String, icon: String)>
  ```

### 🟡 MEDIA PRIORITÀ (prima di nuove feature complesse)

#### 4. Split `dbus_provider.rs` (`providers/dbus/`)

- **Problema:** 848 linee in un file
- **Impatto:** Miglior manutenibilità
- **Stima:** 2 ore
- **Deliverable:**
  ```
  providers/dbus/
  ├── mod.rs
  ├── discovery.rs    # discover_providers, parse_ini
  ├── query.rs        # run_search_streaming, query_one
  ├── icons.rs        # parse_icon_variant, extract_themed, extract_file
  └── types.rs        # SearchProvider, SearchResult, IconData
  ```

#### 5. Split `workspace_bar.rs`

- **Problema:** Logica D-Bus mescolata con UI
- **Impatto:** Miglior testabilità
- **Stima:** 1-2 ore
- **Deliverable:**
  - Estrarre `fetch_workspace_windows`, `activate_window`, `close_window` in `actions/workspace.rs`
  - Mantenere solo UI in `ui/workspace_bar.rs`

#### 6. Unificare tokio runtime

- **Problema:** `workspace_bar.rs` ha runtime locale
- **Impatto:** Rimuove potenziali conflitti
- **Stima:** 30 minuti
- **Deliverable:**
  - Rimuovere `static TOKIO_RT` da `workspace_bar.rs`
  - Usare `global_state::get_tokio_runtime()`

### 🟢 BASSA PRIORITÀ (miglioramenti)

#### 7. Extract `SubprocessRunner` a modulo

- Estrarre in `model/runner.rs`

#### 8. Add `HotReloadable` trait

- Pattern più chiaro per config/theme hot-reload

#### 9. Extract `BindStrategy` registry

- Già buono in `list_factory.rs`, migliorare documentazione

#### 10. Make terminal candidates configurable

- Aggiungere a `Config` la lista dei terminali preferiti

---

## Proposta Nuova Struttura Moduli

```
src/
├── ui/
│   ├── mod.rs
│   ├── window.rs          # Solo orchestrazione top-level
│   ├── context_menu.rs    # NUOVO: Context menu builder
│   ├── list_factory.rs
│   ├── pinned_strip.rs
│   ├── favorites_strip.rs # FUTURO
│   ├── power_bar.rs
│   ├── obsidian_bar.rs
│   ├── workspace_bar.rs   # Solo UI, D-Bus in actions/
│   └── helpers.rs         # NUOVO: make_button, etc.
├── providers/
│   ├── mod.rs             # SearchProvider trait
│   ├── app.rs             # AppProvider
│   ├── calculator.rs      # CalculatorProvider  
│   ├── dbus/
│   │   ├── mod.rs
│   │   ├── discovery.rs
│   │   ├── query.rs
│   │   └── icons.rs
│   └── file.rs            # FileSearchProvider ( FUTURO)
├── model/
│   ├── mod.rs
│   ├── list_model.rs
│   ├── runner.rs          # SubprocessRunner
│   └── items/
├── actions/
│   ├── mod.rs
│   ├── launcher.rs
│   ├── power.rs
│   ├── obsidian.rs
│   ├── file.rs
│   └── workspace.rs       # D-Bus per workspace bar
├── utils/
│   ├── mod.rs
│   ├── clipboard.rs       # NUOVO: unificato
│   ├── desktop.rs         # NUOVO: resolve_desktop_icon condiviso
│   └── path.rs            # expand_home, contract_home
└── ...
```

---

## Conclusione

La codebase è **strutturalmente solida** con buoni pattern esistenti:

- `SearchProvider` trait ben definito
- `SubprocessRunner` pattern efficace
- `BindStrategy` pattern in list_factory
- Config system pulito e testato

**I problemi principali sono:**

1. **Duplicazione codice** - Specialmente nei context menu e utilities
2. **File troppo lunghi** - `dbus_provider.rs`, `workspace_bar.rs` necessitano split
3. **Accoppiamento moderato** - `AppListModel` fa troppo, window.rs ha troppi parametri

**Raccomandazione finale:**

Procedere con i 3 refactoring HIGH PRIORITY per rendere i context menu manutenibili, poi implementare le feature. L'investimento in refactoring ripagherà in manutenibilità a lungo termine.

---

## Appendice: Metriche

| File | Linee | Complessità | Priorità Refactor |
|------|-------|-------------|------------------|
| `ui/window.rs` | 1316 | Media | Media |
| `model/list_model.rs` | 818 | Alta | Media |
| `providers/dbus_provider.rs` | 848 | Alta | Alta |
| `ui/workspace_bar.rs` | 654 | Media | Media |
| `core/config.rs` | 664 | Bassa | Bassa |
| `launcher.rs` | 490 | Bassa | Bassa |
| `ui/list_factory.rs` | 438 | Media | Bassa |
