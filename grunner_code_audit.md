# Grunner — Code Audit (marzo 2026)

## Impressione sommaria

Il progetto ha compiuto un salto di qualità notevole rispetto alle versioni precedenti. La struttura
a moduli è chiara, la separazione tra `providers/`, `actions/`, `model/`, `ui/` e `core/` è sensata,
e il codice complessivamente è idiomatico Rust. I test unitari sono presenti e coprono i
moduli puri (parser, calcolatore, utils), il logging è flessibile e la gestione del debounce è
elegante. Dal punto di vista utente il README è accurato e ben tenuto.

Detto questo, ci sono alcune aree di accoppiamento eccessivo, duplicazione di codice e
decisioni di design che meritano attenzione prima che il progetto cresca ulteriormente. L'audit
che segue affronta ogni area con proposte concrete.

---

## 1. `AppListModel` — God Object

### Il problema

`AppListModel` (648 righe) è un clone (letteralmente, implementa `Clone`) di sé stesso e al
contempo responsabile di:

- mantenere il `ListStore` GTK e la `SingleSelection`
- gestire due timer di debounce separati (`command_debounce` + `search_debounce`)
- coordinare il task-generation counter per le cancellazioni
- fare da facade ai provider (`AppProvider`, `CalculatorProvider`)
- tenere la configurazione (`obsidian_cfg`, `commands`, `blacklist`, `max_results`, `disable_modes`)
- essere passato per clone a tutti gli handler

Il risultato è che ogni modifica alla logica di ricerca richiede di navigare in un unico struct
enorme con campi `pub(crate)` ovunque.

### Refactoring proposto

Estrarre tre responsabilità distinte:

```
SearchState          — generazione corrente, query corrente, cancellazione
DebounceScheduler    — i due timer (command + search) con le stesse signature
ModelConfig          — max_results, debounce_ms, obsidian_cfg, commands, blacklist
```

`AppListModel` diventa allora un coordinatore leggero che delega. La maggior parte dei campi
`Rc<Cell<…>>` e `Rc<RefCell<…>>` duplicati scompaiono.

---

## 2. `schedule_command_with_delay` e `schedule_search_with_delay` sono identiche

### Il problema

```rust
fn schedule_command_with_delay<F>(&self, delay_ms: u32, f: F) { … }
fn schedule_search_with_delay<F>(&self, delay_ms: u32, f: F) { … }
```

Le due funzioni sono **identiche al 100%** tranne per il campo `Rc<RefCell<Option<SourceId>>>`
che aggiornano (`command_debounce` vs `search_debounce`). Questo crea una regola silenziosa:
"se vuoi un terzo tipo di debounce devi copiare ancora".

### Refactoring proposto

```rust
fn schedule_with_debounce<F>(
    slot: &Rc<RefCell<Option<glib::SourceId>>>,
    delay_ms: u32,
    f: F,
)
where
    F: FnOnce() + 'static,
{
    if let Some(id) = slot.borrow_mut().take() { id.remove(); }
    let mut f_opt = Some(f);
    let slot_clone = slot.clone();
    let source_id = glib::timeout_add_local(Duration::from_millis(delay_ms.into()), move || {
        *slot_clone.borrow_mut() = None;
        f_opt.take().map(|f| f());
        glib::ControlFlow::Break
    });
    *slot.borrow_mut() = Some(source_id);
}
```

I quattro metodi pubblici (`schedule_command`, `schedule_search`, …) diventano thin wrapper
di una riga ciascuno.

---

## 3. `load_custom_theme` usa `Box::leak` — memory leak

### Il problema

```rust
fn load_custom_theme(path: Option<&str>) -> &'static str {
    …
    Ok(css) => Box::leak(css.into_boxed_str()),
    …
}
```

Ogni volta che il tema personalizzato viene ricaricato (evento `theme-changed` da
`AppCallbacks`) viene creato un nuovo `&'static str` che non viene mai liberato. Per un
launcher che può essere riavviato centinaia di volte in una sessione è trascurabile, ma è
comunque un leak documentabile e sorprendente.

### Fix proposto

Usare un `OnceLock<String>` o, meglio, tenere la stringa CSS in un `Rc<String>` nel
`ThemeManager` e farla vivere quanto il manager stesso:

```rust
pub struct ThemeManager {
    provider: gtk4::CssProvider,
    custom_css: RefCell<Option<String>>,  // owned, non leaked
}
```

---

## 4. `AppMode` vs `ActiveMode` — doppia enum confusa

### Il problema

Esistono due enum quasi sovrapponibili:

- `AppMode` — usata dalla UI per capire il "modalità di input" (Normal, FileSearch, Obsidian, …)
- `ActiveMode` — usata dal model per capire "come renderizzare gli item" (None, ObsidianAction, ObsidianFile, …)

La distinzione è reale (una guida l'input, l'altra il rendering) ma **non è documentata né
resa esplicita**. Chi legge `command_handler.rs` deve tenere in testa la mappa mentale
`AppMode::Obsidian → ActiveMode::ObsidianFile` o `ActiveMode::ObsidianAction` a seconda
dell'argomento.

### Proposta

Aggiungere un commento doc esplicito in cima a entrambe le enum che spiega la relazione,
oppure consolidarle in un'unica enum con una struttura a due livelli:

```rust
pub enum SearchMode {
    Normal,
    FileSearch { grep: bool },
    Obsidian { submode: ObsidianSubmode },
    CustomScript,
}

pub enum ObsidianSubmode {
    Actions,       // :ob senza argomento
    FileSearch,    // :ob con argomento
    Grep,          // :obg
}
```

Questo elimina la necessità di mappare mentalmente tra le due enum.

---

## 5. `build_ui` — troppo lunga, `#[allow(clippy::too_many_lines)]`

### Il problema

`ui/window.rs::build_ui` (con il suo `#[allow(clippy::too_many_lines)]`) è la funzione
principale di costruzione UI e tende ad attrarre nuova logica ogni volta che viene aggiunta
una feature. Al momento ci sono dentro:

- setup CSS/tema
- creazione finestra
- setup callbacks di hot-reload
- costruzione layout (delegata a `build_main_layout`)
- setup segnali entry (delegata)
- setup tastiera
- setup lista
- setup context menu
- loading in background

Molte delle sotto-funzioni (`connect_search_signals`, `setup_keyboard_controller`, …) sono
già estratte correttamente. Il problema è che `build_ui` rimane un driver che tiene
**14 variabili locali** e le passa esplicitamente tra di esse.

### Proposta

Introdurre un `UiContext` o `WindowBuilder` che raggruppa lo stato condiviso durante la
costruzione:

```rust
struct WindowBuilder<'a> {
    app: &'a Application,
    cfg: &'a Config,
    model: AppListModel,
    current_mode: Rc<Cell<AppMode>>,
    all_apps: Rc<RefCell<Vec<DesktopApp>>>,
    pinned_apps: Rc<RefCell<Vec<String>>>,
    dragging: Rc<Cell<bool>>,
    display: gdk::Display,
}
```

I metodi `setup_css`, `build_layout`, `connect_signals`, `start_loading` diventano metodi
di `WindowBuilder` e non devono più passarsi `cfg`, `model`, `pinned_apps` etc. come
argomenti separati.

---

## 6. `ObsidianConfig::default()` sempre presente — logica di default errata

### Il problema

In `Config::default()`:

```rust
obsidian: Some(ObsidianConfig::default()),
```

`ObsidianConfig::default()` ha `vault = ""`. Questo significa che Obsidian è sempre
"configurato" (è `Some`), ma con un vault vuoto. La `validated_vault_path()` nel
`CommandHandler` deve poi fare un ulteriore check sull'esistenza del path.

### Effetto collaterale

`show_obsidian_bar()` ritorna `true` anche se l'utente non ha mai configurato Obsidian, e la
UI mostra la barra Obsidian con pulsanti che falliranno silenziosamente (mostreranno solo
l'errore "Vault path does not exist").

### Fix proposto

Cambiare `Config::default()` a `obsidian: None` e usare `None` come segnale "non
configurato". La settings window può inizializzare un `ObsidianConfig` vuoto solo quando
l'utente apre la tab Obsidian.

---

## 7. `AppProvider` — fast-path e fuzzy-path danno ordinamenti inconsistenti

### Il problema

```rust
fn search_apps_optimized(…) -> Vec<&'a DesktopApp> {
    // Fast path: prefix/contains match, NON ordinato per score
    let prefix_results: Vec<_> = apps.iter()
        .filter(|app| app.name.to_lowercase().starts_with(…) || …contains(…))
        .take(max_results)
        .collect();

    if !prefix_results.is_empty() { return prefix_results; }

    // Fuzzy fallback: ordinato per score
    …
}
```

Il fast-path ritorna i risultati nell'ordine in cui le app sono state caricate
(alfabetico dal `scan_apps`), **non per rilevanza**. Il fuzzy-path li ordina per score. Un
utente che digita "fi" vedrà le app in ordine alfabetico, ma aggiungendo una lettera che
esce dal fast-path vedrà improvvisamente un riordinamento. L'esperienza è incoerente.

### Fix proposto

Aggiungere un quick-score anche nel fast-path:

```rust
let mut scored: Vec<_> = prefix_results.iter()
    .map(|app| {
        let score = if app.name.to_lowercase().starts_with(&query_lower) { 100 }
                    else { 50 };  // contains
        (score, *app)
    })
    .collect();
scored.sort_by(|a, b| b.0.cmp(&a.0));
```

Oppure eliminare il fast-path e fidarsi del fuzzy matcher (che è già velocissimo su liste
di poche centinaia di app).

---

## 8. `launch_app` — shell injection latente

### Il problema

```rust
cmd.arg("-e").arg("sh").arg("-c").arg(&clean);
```

`clean` è il risultato di `clean_exec(exec)` che rimuove i field code (`%f`, `%U`, …) ma
**non fa escaping** dei metacaratteri shell. Se un `.desktop` file ha un `Exec` con
caratteri come `$(...)`, `` ` ``, o `|`, questi vengono passati a `sh -c` verbatim.

In pratica è un rischio basso (i file `.desktop` di sistema sono fidati), ma i `.desktop`
installati dall'utente in `~/.local/share/applications/` potrebbero essere costruiti
malevolmente.

### Fix proposto

Usare `Command::arg` con splitting manuale invece di `sh -c`:

```rust
let parts: Vec<&str> = clean.split_whitespace().collect();
if let Some((prog, args)) = parts.split_first() {
    let mut cmd = std::process::Command::new(prog);
    cmd.args(args);
}
```

Per le app che richiedono shell expansion (rare) si può mantenere `sh -c` come fallback
opzionale, ma il path principale non dovrebbe passare per la shell.

---

## 9. Variabile `_has_minus` inutilizzata in `calculator.rs`

### Il problema

```rust
let _has_minus = trimmed.contains('-');
```

Viene computata ma usata solo tramite `has_non_leading_minus` e `has_multiple_minuses` che
la rimpiazzano. Il `_` è un segnale che anche il compilatore sta segnalando qualcosa.

### Fix

Rimuovere la riga, visto che l'informazione è già contenuta nei due flag derivati.

---

## 10. `poll_apps` — busy-polling su idle con firma in crescita

### Il problema

```rust
fn poll_apps(
    rx: Receiver<Vec<DesktopApp>>,
    model: AppListModel,
    all_apps: Rc<…>,
    pinned_strip: GtkBox,
    pinned_apps: Rc<…>,
    window: ApplicationWindow,
    dragging: Rc<Cell<bool>>,
) { … }
```

La funzione ha 7 argomenti e si richiama ricorsivamente tramite `glib::idle_add_local_once`.
Ogni nuova feature che dipende dal momento "le app sono caricate" (es. workspace bar) rischia
di aggiungere un altro argomento.

### Refactoring proposto

Usare il pattern callback-on-ready:

```rust
fn start_background_loading(cfg: &Config, on_loaded: impl Fn(Vec<DesktopApp>) + 'static) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || { tx.send(launcher::load_apps(&dirs)).ok(); });
    glib::idle_add_local_once(move || {
        match rx.try_recv() {
            Ok(apps) => on_loaded(apps),
            Err(TryRecvError::Empty) => { /* reschedule */ }
            …
        }
    });
}
```

La closure `on_loaded` cattura solo ciò che le serve, e la firma di `start_background_loading`
rimane stabile.

---

## 11. Campo `pub store` e `pub selection` in `AppListModel`

### Il problema

`store` e `selection` sono campi `pub` in `AppListModel`. Qualsiasi codice esterno può
fare `model.store.remove_all()` o `model.selection.set_selected(0)` bypassando la logica
di debounce, generation-tracking e mode-setting.

### Fix

Esporre solo metodi semantici:

```rust
pub fn clear(&self) { … }              // rimuove items + resetta selection
pub fn selected_item(&self) -> Option<glib::Object> { … }
pub fn selected_position(&self) -> u32 { … }
```

Rendere `store` e `selection` `pub(crate)` almeno, o meglio `private`.

---

## 12. README lag — feature non documentate

Il README non menziona:
- la **pinned strip** (Alt+1…Alt+9 per lanciare app pinnate)
- la **workspace bar** (barra delle finestre aperte sul workspace corrente)
- il **context menu** sulla lista (click destro)
- la **tab Theme** nella settings window (selezione tema da GUI)
- la flag `--list-providers` da CLI
- la modalità `:sh` per script custom

L'architettura table nel README elenca file che non esistono più (`actions.rs`, `config.rs`,
`logging.rs` a livello root) mentre la struttura attuale è a moduli (`actions/mod.rs`,
`core/config.rs`, ecc.).

---

## 13. `disable_modes` — flag duplicata tra `Config` e `AppListModel`

`Config::disable_modes` viene copiata in `AppListModel::disable_modes` al momento della
costruzione ma non viene aggiornata da `apply_config`. Se l'utente abilitasse o disabilitasse
la modalità simple via settings senza riavviare, il model userebbe il valore iniziale.
Attualmente la settings window non espone questa opzione, quindi non è un bug attivo, ma è
una inconsistenza potenziale.

### Fix

Rimuovere `disable_modes` da `AppListModel` e leggerlo direttamente da un `Config` condiviso,
oppure assicurarsi che `apply_config` lo aggiorni.

---

## 14. `CommandHandler` — accoppiamento circolare con `AppListModel`

`CommandHandler` prende `&AppListModel` e chiama metodi come `set_active_mode`,
`bump_task_gen`, `append_store_item`, `remove_all_store_items`. Questo crea un ciclo
logico: `AppListModel::handle_colon_command` crea un `CommandHandler` che poi richiama
`AppListModel` per modificare lo store.

L'estrazione del `CommandHandler` è già un miglioramento rispetto al passato, ma la
dipendenza è ancora circolare a livello di struttura.

### Proposta a lungo termine

Definire un trait `CommandSink` che `AppListModel` implementa:

```rust
pub trait CommandSink {
    fn set_mode(&self, mode: ActiveMode);
    fn clear(&self);
    fn push(&self, item: &impl IsA<glib::Object>);
    fn bump_gen(&self) -> u64;
    fn schedule<F: FnOnce() + 'static>(&self, f: F);
}
```

`CommandHandler<S: CommandSink>` dipende dal trait, non dalla struttura concreta.
Diventa testabile in isolamento con un mock.

---

## Riepilogo priorità

| # | Area | Impatto | Effort |
|---|------|---------|--------|
| 3 | `Box::leak` in `load_custom_theme` | Bug reale (leak) | Basso |
| 9 | Variabile `_has_minus` inutilizzata | Pulizia | Minimo |
| 6 | `ObsidianConfig::default()` sempre `Some` | UX bug latente | Basso |
| 2 | Debounce duplicato | DRY / manutenibilità | Basso |
| 7 | Ordinamento inconsistente in `AppProvider` | UX | Medio |
| 8 | Shell injection in `launch_app` | Sicurezza (basso rischio pratico) | Medio |
| 11 | Campi `pub store`/`selection` | Encapsulamento | Medio |
| 12 | README lag | Documentazione | Medio |
| 13 | `disable_modes` non aggiornato in `apply_config` | Bug latente | Basso |
| 10 | `poll_apps` firma in crescita | Manutenibilità | Medio |
| 1 | `AppListModel` god object | Architettura | Alto |
| 4 | `AppMode` vs `ActiveMode` confusione | Leggibilità | Medio |
| 5 | `build_ui` troppo lunga | Manutenibilità | Alto |
| 14 | `CommandHandler` circolare | Architettura | Alto |
