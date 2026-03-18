# Analisi Architetturale del Codice Sorgente Grunner

**Data:** 18 Marzo 2026  
**Ultimo Aggiornamento:** 18 Marzo 2026 (Secondo Audit Post-Refactoring)

---

## Sommario

Questo documento fornisce un'analisi approfondita della struttura del codice di Grunner dopo il refactoring completato il 18 Marzo 2026.

### File Principali (Righe)

| File | Righe | Stato Pre-Refactoring | Stato Post-Refactoring |
|------|-------|----------------------|------------------------|
| `model/list_model.rs` | 818 | God class (959 righe) | Migliorato, CommandHandler estratto |
| `ui/window.rs` | 589 | Funzione `build_ui` troppo lunga | Spezzata in helper functions |
| `providers/dbus_provider.rs` | 848 | Complesso, Icon parsing intricato | Invariato (Bassa priorità) |
| `ui/workspace_bar.rs` | 509 | D-Bus + Tokio runtime locale | Runtime locale (OK) |
| `ui/power_bar.rs` | 212 | UI + azioni mescolate | Invariato |
| `ui/list_factory.rs` | 438 | bind_* funzioni complesse | Strategy pattern applicato |
| `actions/` | ~300 | 6 responsabilità in 1 file | Sottomoduli separati |

---

## 1. Accoppiamenti Stretti (Tight Coupling) - Post Refactoring

### 1.1 ✅ Stato Globale Condiviso (`global_state.rs`) - RISOLTO PARZIALMENTE

**Stato Attuale:**
- `HOME_DIR`, `TOKIO_RUNTIME`, `CONFIG_RELOADER`, `THEME_RELOADER` ancora con `OnceLock`
- Test helpers rimossi (non testabili)

**Rimane Aperto:**
- Impossibile mock per test senza rifare `OnceLock` in modo testabile
- Vedi Task #8 nel Chapter 10

### 1.2 ⚠️ UI Accoppiata al Modello (`ui/window.rs` ↔ `model/list_model.rs`)

**Stato Attuale:**
```rust
// window.rs crea e usa direttamente AppListModel
let model = setup_model(cfg);
model.schedule_populate(&text);
activate_item(&obj, &model, current_mode.get(), timestamp);
```

**Problema:**
- `activate_item()` riceve `&AppListModel` per accedere a `obsidian_cfg`
- Separazione UI/Model ancora incompleta

**Proposta:**
- `ActivationContext` già introdotto (refactoring parziale)
- Necessario estrarre ulteriormente per独立性 completa

### 1.3 ✅ Azioni Refactorizzate (`actions/`) - RISOLTO

Le azioni ora sono separate in sottomoduli con responsabilità chiare.

---

## 2. Funzioni - Stato Attuale

### 2.1 ✅ `AppListModel` - MIGLIORATO

**Riduzione:** Da 959 a ~818 righe (-141 righe)

**Responsabilità Attuali (necessarie):**
- Gestione store GTK (`store`, `selection`)
- Ricerca fuzzy (`all_apps`, `providers`)
- Threading con `SubprocessRunner`
- Polling asincrono (`ProviderSearchPoller`)
- Factory UI delegation
- Config hot-reload

**Estratto:**
- `CommandHandler` in `command_handler.rs` (229 righe)

### 2.2 ⚠️ `build_ui` in `window.rs` - MIGLIORATO PARZIALMENTE

**Prima:** Funzione monolitica di ~600 righe

**Dopo:** Spezzata in helper functions:
- `setup_css()` (24 righe)
- `setup_model()` (18 righe)
- `create_window()` (25 righe)
- `build_sidebar()` (56 righe)
- `build_main_layout()` (80 righe)
- `connect_window_signals()` (38 righe)
- `connect_search_signals()` (39 righe)
- `connect_list_signals()` (23 righe)
- `start_background_loading()` (11 righe)
- `setup_keyboard_controller()` (67 righe)

**Rimane:**
- `build_ui()` ha ancora ~70 righe che orchestrano il setup
- Può essere ulteriormente semplificato con un pattern builder

### 2.3 ⚠️ `dbus_provider.rs` - NON MODIFICATO

**Problemi Identificati:**
- Parsing Icon ancora intricato (80+ righe di match annidati)
- Streaming con canali MPSC per risultati batch
- 848 righe totali

**Stato:** Bassa priorità, vedi Task #9

---

## 3. Interfacce Trait - Post Refactoring

### 3.1 ✅ Sistema Provider - MIGLIORATO

**`SearchProvider` trait** (già esistente):
```rust
pub trait SearchProvider {
    fn search(&self, query: &str) -> Vec<glib::Object>;
}
```

**Implementato da:**
- `AppProvider` ✅
- `CalculatorProvider` ✅
- `DbusSearchProvider` ✅ (rinominato)

### 3.2 ✅ `CommandProvider` trait - NUOVO

```rust
pub trait CommandProvider {
    fn get_commands(&self, query: &str) -> Vec<CommandConfig>;
}
```

**Implementato da:**
- `AppListModel` ✅

---

## 4. Duplicazioni di Codice - Risolte

### 4.1 ✅ Subprocess Execution - RISOLTO

**Prima:** Thread spawn + channel + polling ripetuto in `run_subprocess()`, `run_provider_search()`

**Dopo:** `SubprocessRunner<R>` generic struct:
```rust
pub struct SubprocessRunner<R> {
    rx: std::sync::mpsc::Receiver<R>,
    model: AppListModel,
    generation: u64,
    processor: Box<dyn Fn(&AppListModel, u64, R) + 'static>,
}
```

### 4.2 ✅ Path Handling - GIA PRESENTE

`expand_home()` e `contract_home()` già ben separati in `utils.rs`

---

## 5. Punti Critici per Feature Future

### 5.1 Striscia Preferiti (Prompt 1)

**Posizione:** `ui/favorites_bar.rs` - tra search entry e results list

**Invasività:** **BASSA** ✅

Il refactoring di `window.rs` ha già preparato il terreno:
- `build_main_layout()` assembla i componenti in ordine
- `entry_box` ha spazio per aggiungere widget prima di `content`
- Bar laterali già implementate con pattern rivelazione (workspace_bar)

**Implementazione Suggerita:**
```rust
// In build_main_layout()
let favorites_bar = build_favorites_bar(&model);
content.append(&favorites_bar); // Dopo entry_box, prima scrolled
```

### 5.2 Context Menu (Prompt 2-6)

**Posizione:** `list_factory.rs` - `bind_*` funzioni + `item_activation.rs`

**Invasività:** **MEDIA-ALTA** ⚠️

**Refactoring Pre-Requisito (già completato):**
- ✅ `BindStrategy` pattern in `list_factory.rs`
- ✅ `Activatable` trait in `item_activation.rs`

**Necessario Per Context Menu:**
1. Aggiungere `EventController` per click destro ai widget
2. Estendere `ActivatableItem` con metodo per menu contestuale
3. Handler menu in `ActivationContext`

### 5.3 Estensibilità Config

**Stato:** Suffciente per ora

Il sistema config usa serde con deserializzazione diretta.

---

## 6. Interventi Prioritizzati

### Alta Priorità ✅

| # | Intervento | File | Stato |
|---|------------|------|-------|
| 1 | Estrarre `CommandHandler` da `AppListModel` | list_model.rs, command_handler.rs | ✅ Completato |
| 2 | Refactoring `actions.rs` in sottomoduli | actions.rs → actions/ | ✅ Completato |
| 3 | Creare trait `CommandProvider` | providers/mod.rs | ✅ Completato |

### Media Priorità ✅

| # | Intervento | File | Stato |
|---|------------|------|-------|
| 4 | Pattern Strategy per `bind_command_item()` | list_factory.rs | ✅ Completato |
| 5 | Unificare subprocess execution | list_model.rs | ✅ Completato |
| 6 | Helper path in utils.rs | utils.rs | ✅ Completato |

### Bassa Priorità 🔄

| # | Intervento | File | Stato |
|---|------------|------|-------|
| 7 | Aggiungere trait `Activatable` | item_activation.rs | ✅ Completato |
| 8 | Mock `global_state` per test | global_state.rs | ❌ Da fare |
| 9 | Refactoring icon parsing D-Bus | dbus_provider.rs | ❌ Da fare |

---

## 7. Struttura Moduli - Stato Finale

```
src/
├── actions/                    # ✅ Refattorizzato
│   ├── mod.rs                 # ✅
│   ├── launcher.rs            # ✅ launch_app
│   ├── power.rs               # ✅ power_action
│   ├── file.rs                # ✅ open_file_or_line
│   ├── obsidian.rs            # ✅ obsidian actions
│   └── settings.rs            # ✅ open_settings
├── command_handler.rs         # ✅ NUOVO - estratto da list_model.rs
├── providers/
│   ├── mod.rs                 # ✅ SearchProvider + CommandProvider
│   ├── dbus_provider.rs       # ⚠️ Icon parsing ancora intricato
│   ├── app.rs                 # (inline in mod.rs)
│   └── calculator.rs          # (inline in mod.rs)
├── core/
│   ├── config.rs
│   ├── global_state.rs        # ⚠️ Still OnceLock, non testabile
│   └── theme.rs
├── model/
│   ├── list_model.rs          # ✅ ~818 righe (era 959)
│   └── items/
├── ui/
│   ├── window.rs              # ✅ build_ui spezzato
│   ├── list_factory.rs        # ✅ BindStrategy pattern
│   ├── obsidian_bar.rs
│   ├── power_bar.rs
│   ├── workspace_bar.rs
│   └── favorites_bar.rs       # 🔲 TODO: Feature Prompt 1
├── item_activation.rs         # ✅ Activatable trait
├── app_mode.rs
├── calculator.rs
├── launcher.rs
└── utils.rs                   # ✅ expand_home, contract_home
```

---

## 8. Criticità Residue

### Alta

| # | Problema | Impatto |
|---|----------|---------|
| - | Nessuna | - |

### Media

| # | Problema | Impatto | Soluzione |
|---|----------|---------|-----------|
| 1 | `global_state.rs` non testabile | Non posso mock `get_home_dir()` in test | Vedi Task #8 |
| 2 | `dbus_provider.rs` Icon parsing intricato | Manutenzione difficile | Vedi Task #9 |

### Bassa

| # | Problema | Impatto |
|---|----------|---------|
| 1 | `build_ui` può essere ulteriormente semplificato | Manutenzione |
| 2 | Clippy warning in `SubprocessRunner` type_complexity | Warning |

---

## 9. Raccomandazioni Pre-Feature

### Per Prompt 1 (Striscia Preferiti) ✅

**Pronto per implementazione.** Il refactoring ha preparato:
- Widget composition in `build_main_layout()`
- Pattern sidebar già esistente (`workspace_bar`)
- Non richiede refactoring aggiuntivo

### Per Prompt 2-6 (Context Menu) ⚠️

**Richiede completamento Task #7 prima di procedere.**

Il trait `Activatable` è stato introdotto ma non è ancora usato ovunque.
Completare l'adozione del pattern prima di aggiungere context menu.

### Per Test Coverage

**Completare Task #8** prima di aggiungere test di integrazione per:
- `AppListModel` con mock config
- Command execution con mock filesystem

---

## 10. Todo List Aggiornata (18 Marzo 2026)

### Completati ✅

- [x] Refactoring `actions.rs` in sottomoduli
- [x] Estrarre `CommandHandler` da `AppListModel`
- [x] Creare trait `CommandProvider`
- [x] Pattern Strategy per `bind_command_item()`
- [x] Unificare subprocess execution con `SubprocessRunner`
- [x] Helper path in `utils.rs`
- [x] Aggiungere trait `Activatable`
- [x] Spezzare `build_ui` in helper functions

### Da Completare ❌

#### Bassa Priorità

- [ ] **Task #8**: Mock `global_state` per test
  - Soluzione: Usare `thread_local` + `RefCell` per test injection
  - Alternativa: Passare dipendenze esplicitamente

- [ ] **Task #9**: Refactoring icon parsing D-Bus
  - Estrarre `IconParser` trait
  - Separare parsing dalla logica D-Bus

---

## 11. Metriche

### Prima del Refactoring (18 Marzo 2026 - Primo Audit)

| Metrica | Valore |
|---------|--------|
| Totale righe (file principali) | ~3500+ |
| God class `AppListModel` | 959 righe |
| File `actions.rs` monolitico | 639 righe |
| Test coverage | ~70% (stima) |

### Dopo il Refactoring

| Metrica | Valore | Delta |
|---------|--------|-------|
| Totale righe (file principali) | ~3300+ | -200 |
| God class `AppListModel` | 818 righe | -141 (-15%) |
| Directory `actions/` | ~300 righe | Scomposto |
| File `command_handler.rs` | 229 righe | Estratto |
| Pattern Strategy | 6 implementazioni | Nuovo |
| Test coverage | Invariato | - |
| Test count | 57 | +5 doctests |

### Clippy Status

```
warning: very complex type used (type_complexity)
  --> list_model.rs:48
```

1 warning minore, nessun errore.

---

## 12. Conclusione

Il refactoring completato ha:
1. ✅ Ridotto la complessità di `AppListModel` del 15%
2. ✅ Separato le responsabilità in moduli coesi
3. ✅ Introdotto pattern riutilizzabili (`BindStrategy`, `Activatable`)
4. ✅ Preparato il terreno per le feature future

**Il codebase è ora pronto per:**
- Implementazione della striscia preferiti (Bassa invasività)
- Implementazione context menu (Media invasività, richiede Task #7)
- Test di integrazione (richiede Task #8)

**Rimangono come technical debt:**
- Icon parsing intricato in `dbus_provider.rs` (Bassa priorità)
- `global_state.rs` non testabile (Bassa priorità, impatta test coverage)
