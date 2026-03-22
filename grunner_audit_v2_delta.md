# Grunner — Audit Delta (v2, marzo 2026)

Rispetto all'audit precedente, 11 dei 14 punti sono stati risolti correttamente.
Questo documento copre solo i problemi residui e i nuovi problemi introdotti dal refactoring.

---

## Ancora aperti dall'audit precedente

### #5 — `build_ui` / `#[allow(clippy::too_many_lines)]`

`WindowContext` è un passo nella direzione giusta e ha ridotto la logica visibile in
`build_ui`, ma il flag è ancora lì. Non urgente, ma il debito cresce ad ogni nuova feature.

### #12 — README lag

Non verificabile dal codice, ma quasi certamente ancora indietro rispetto alle feature
aggiunte nelle ultime sessioni (pinned strip, workspace bar, context menu, tab tema,
`:sh`, `--list-providers`).

---

## Nuovi problemi introdotti dal refactoring

### N1 — `CommandSink::bump_and_schedule` non ha il generation check (bug potenziale)

Il metodo sul trait fa:

```rust
fn bump_and_schedule<F: FnOnce() + 'static>(&self, f: F) {
    self.bump_task_gen();
    AppListModel::schedule_command(self, f);  // ← f viene eseguita SEMPRE
}
```

Il metodo `pub(crate)` diretto su `AppListModel` fa invece:

```rust
let generation = self.bump_task_gen();
let model_clone = self.clone();
self.schedule_command(move || {
    if model_clone.state.task_gen() == generation { f(); }  // ← protetto da stale
});
```

Sono due implementazioni con lo stesso nome ma semantica diversa. In questo momento non è
un bug attivo perché `AppCommandHandler` (il type alias concreto) chiama i metodi diretti
sul modello, non quelli del trait. Ma qualunque futuro implementatore di `CommandSink` che
chiami `sink.bump_and_schedule(f)` non avrà la protezione contro risultati stale.

**Fix:** o rimuovere `bump_and_schedule` dal trait e lasciarlo solo come metodo concreto
su `AppListModel`, oppure aggiornare l'implementazione nel trait per includere il check:

```rust
fn bump_and_schedule<F: FnOnce() + 'static>(&self, f: F) {
    let gen = self.bump_gen();
    let sink = self.clone();
    self.schedule(move || {
        // I mock di test possono esporre task_gen attraverso il trait se necessario
        f();  // oppure: if sink.current_gen() == gen { f(); }
    });
}
```

Se il generation check deve essere parte del contratto del trait, aggiungere
`fn current_gen(&self) -> u64` a `CommandSink`.

---

### N2 — `CommandHandler::model` è `pub`

```rust
pub struct CommandHandler<T: CommandSink> {
    pub model: T,   // ← pub
```

Probabilmente messo `pub` per comodità futura (es. test), ma espone l'implementazione
interna al di fuori del crate. `pub(crate)` è sufficiente.

---

### N3 — `ThemeManager` ricreato ad ogni `theme-changed`, i vecchi `CssProvider` si accumulano

```rust
self.callbacks.connect_theme_changed(move |_| {
    let mut theme_manager = crate::core::theme::ThemeManager::new(); // nuovo ogni volta
    theme_manager.apply(…, &display);
});
```

`ThemeManager::apply` chiama `gtk4::style_context_add_provider_for_display` ma non rimuove
mai il provider precedente. Ad ogni cambio tema si aggiunge un nuovo `CssProvider` alla
display senza togliere il vecchio. Le regole CSS si accumulano e quelle con priorità più
alta (aggiunte dopo) sovrascrivono le precedenti, ma i provider obsoleti rimangono in
memoria e nella pipeline di stile GTK per tutta la vita della sessione.

Il fix del `Box::leak` (punto #3 del vecchio audit) è corretto — ora `custom_css` è owned.
Il problema residuo è il ciclo di vita del manager stesso.

**Fix:** spostare il `ThemeManager` dentro `WindowContext` e riusare lo stesso provider:

```rust
struct WindowContext {
    …
    theme_manager: ThemeManager,  // vive quanto la finestra
}
```

```rust
// In ThemeManager, prima di caricare il nuovo CSS:
pub fn apply(&mut self, mode: ThemeMode, custom_path: Option<&str>, display: &gdk::Display) {
    // Rimuovi il provider precedente prima di aggiungerne uno nuovo
    gtk4::style_context_remove_provider_for_display(display, &self.provider);
    …
    gtk4::style_context_add_provider_for_display(display, &self.provider, PRIORITY);
}
```

In alternativa, chiamare `self.provider.load_from_data(new_css)` sullo stesso provider
già registrato — GTK applica automaticamente il nuovo CSS senza dover rimuovere e
ri-aggiungere il provider alla display.

---

## Riepilogo priorità

| # | Problema | Impatto | Effort |
|---|----------|---------|--------|
| N3 | `ThemeManager` ricreato ad ogni cambio tema | Memory leak / CSS stacking | Basso |
| N1 | `CommandSink::bump_and_schedule` senza generation check | Bug latente | Basso |
| N2 | `CommandHandler::model` è `pub` | Encapsulamento | Minimo |
| #5 | `build_ui` ancora troppo lunga | Manutenibilità | Alto |
| #12 | README lag | Documentazione | Medio |
