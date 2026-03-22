# Grunner — Audit v5

Solo `context_menu.rs` è cambiato (P6 risolto con `selection.selected()` diretto ✅).
I problemi qui sotto sono tutti in `actions/workspace.rs` e `ui/workspace_bar.rs`,
moduli che non avevano ancora ricevuto un'analisi dedicata.

---

## W1 — `on_change_cell`: reference cycle Rc garantito

**File:** `src/ui/workspace_bar.rs`

```rust
let on_change_cell: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(...None...);
let oc_cell = on_change_cell.clone();

let on_change: Rc<dyn Fn()> = Rc::new(move || {
    if let Some(ref cb) = *oc_cell.borrow() { … }  // on_change cattura oc_cell
});
on_change_cell.borrow_mut().replace(on_change.clone());  // on_change_cell contiene on_change
```

`on_change` cattura `oc_cell` (clone di `on_change_cell`), che a sua volta contiene
`on_change`. Il contatore di riferimento non scende mai a zero: leak per tutta la durata
della sessione.

**Fix:** la closure non ha bisogno di referenziarsi. Basta passare direttamente i widget
di cui ha bisogno senza il livello di indirezione:

```rust
let scroll_r = scroll.clone();
let buttons_r = buttons_box.clone();
let window_r = window.clone();

let on_change: Rc<dyn Fn()> = Rc::new(move || {
    spawn_refresh_delayed(&scroll_r, &buttons_r, &window_r, /* rimosso: cb */ 350);
});
```

`spawn_refresh_delayed` non ha bisogno di `on_change` come argomento perché può costruirla
internamente con gli stessi widget. Oppure, se il ciclo "refresh chiama se stesso dopo
close" è necessario, usare `glib::WeakRef` per spezzare il ciclo.

---

## W2 — `activate_window` e `close_window` aprono una nuova connessione D-Bus ogni volta

**File:** `src/actions/workspace.rs`

```rust
pub async fn activate_window(id: u64) {
    let Ok(conn) = Connection::session().await else { return; };  // nuova connessione
    …
}

pub async fn close_window(id: u64) {
    let Ok(conn) = Connection::session().await else { return; };  // nuova connessione
    …
}
```

Ogni click su un pulsante della workspace bar apre una connessione D-Bus dalla sessione,
poi la scarta. `fetch_workspace_windows` fa lo stesso. Al contrario, `dbus/query.rs` usa
correttamente `get_or_init_conn()` con `OnceLock`.

**Fix:** applicare lo stesso pattern di `dbus/query.rs`:

```rust
async fn get_workspace_conn() -> zbus::Result<Connection> {
    static CONN: OnceLock<Connection> = OnceLock::new();
    if let Some(c) = CONN.get() { return Ok(c.clone()); }
    let conn = Connection::session().await?;
    Ok(CONN.get_or_init(|| conn).clone())
}
```

---

## W3 — `close_all_windows` è sequenziale

**File:** `src/actions/workspace.rs`

```rust
pub async fn close_all_windows(ids: Vec<u64>) {
    for id in ids {
        close_window(id).await;   // aspetta risposta D-Bus prima di procedere
    }
}
```

Con N finestre aperte, il tempo di esecuzione è O(N × latenza_dbus). Con la connessione
condivisa di W2, le chiusure potrebbero essere inviate in parallelo:

```rust
pub async fn close_all_windows(ids: Vec<u64>) {
    let futs: Vec<_> = ids.into_iter().map(close_window).collect();
    futures::future::join_all(futs).await;
}
```

---

## W4 — `WindowInfo.id` è `u64` ma il proxy D-Bus accetta `u32` — fallback silenzioso sbagliato

**File:** `src/actions/workspace.rs`

```rust
fn activate(&self, win_id: u32) -> zbus::Result<()>;

// In activate_window:
.activate(u32::try_from(id).unwrap_or(u32::MAX)).await;
```

Se `id > u32::MAX` (teoricamente possibile con certi compositor), il fallback invia
`u32::MAX` come window ID, che è un ID quasi certamente inesistente ma valido per il
proxy — la chiamata parte senza errore e poi probabilmente fallisce silenziosamente
lato GNOME Shell.

Il tipo `id` in `RawWindowEntry` è deserializzato da JSON come `u64` ma l'estensione
`window-calls` restituisce in realtà valori `u32`. La causa è che `serde_json` su
numeri interi usa `u64` di default. Deserializzare direttamente come `u32` elimina
l'ambiguità:

```rust
struct RawWindowEntry {
    id: u32,   // corrisponde al tipo reale dell'estensione
    …
}
```

E propagare `u32` in tutto il modulo (`WindowInfo.id: u32`,
`activate_window(id: u32)`, ecc.) invece di usare `u64` con cast.

---

## W5 — `save_pinned_apps` legge l'intera config da disco ad ogni modifica

**File:** `src/ui/pinned_strip.rs`

```rust
pub fn save_pinned_apps(pinned_apps: &[String]) {
    let mut cfg = config::load();   // legge da disco
    cfg.pinned_apps = pinned_apps.to_vec();
    save_config(&cfg);              // riscrive su disco
}
```

Il pattern read-modify-write su file senza lock introduce una race condition: se
l'utente salva le impostazioni dalla settings window mentre sta trascinando un'app
pinnata, uno dei due salvataggi sovrascriverà le modifiche dell'altro. In pratica raro
ma strutturalmente scorretto.

**Fix:** passare la `Config` corrente come parametro invece di ricaricarla da disco,
oppure esporre un metodo `Config::update_pinned_apps` che aggiorna solo il campo in
memoria senza I/O.

---

## W6 — `spawn_refresh_delayed` crea un thread OS per ogni aggiornamento

**File:** `src/ui/workspace_bar.rs`

```rust
fn spawn_refresh_delayed(…) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(delay_ms));
        let rt = get_tokio_runtime();
        let windows = rt.block_on(ws::fetch_workspace_windows());
        tx.send(windows);
    });
}
```

Ogni volta che la finestra diventa visibile, o che si chiude una finestra nel workspace
bar, viene creato un thread OS. Se l'utente apre e chiude Grunner rapidamente più volte
in sequenza, i thread si accumulano (ognuno con il suo sleep + block_on in coda).

La stessa cosa si ottiene senza creare thread usando il tokio runtime già disponibile:

```rust
fn spawn_refresh_delayed(…, delay_ms: u64) {
    glib::spawn_future_local(async move {
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
        let windows = ws::fetch_workspace_windows().await;
        // aggiorna UI direttamente nel main thread
        populate(…);
    });
}
```

---

## Riepilogo priorità

| # | Problema | Impatto | Effort |
|---|----------|---------|--------|
| W1 | Rc cycle in `on_change_cell` | Memory leak (lifetime sessione) | Medio |
| W4 | `WindowInfo.id` u64/u32 mismatch con fallback silenzioso | Correttezza | Basso |
| W2 | Nuova connessione D-Bus per ogni operazione workspace | Performance | Basso |
| W5 | `save_pinned_apps` legge config da disco ad ogni salvataggio | Race condition | Medio |
| W3 | `close_all_windows` sequenziale | Performance (marginale) | Basso |
| W6 | Thread OS per ogni refresh workspace | Resource usage | Medio |
