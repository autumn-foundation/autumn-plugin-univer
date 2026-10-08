# autumn-plugin-univer

[Univer](https://univer.ai) spreadsheets for [Autumn](https://autumn-web.app)
0.8 apps. The server renders the page. Univer runs in the browser. Your app
keeps the data.

- No npm, no bundler and no CDN in your app. The crate holds the Univer
  1.0.3 bundle.
- Typed Rust data in and out: `Workbook`, `Sheet`, `Cell`.
- Autosave to your route, with the Autumn CSRF token.
- Works with the default Autumn CSP. The markup has no inline script and
  no inline style (see "Security" for nonce mode).
- Works with htmx: swapped sheets mount, removed sheets dispose.

## Quickstart

```toml
[dependencies]
autumn-plugin-univer = "0.1"
```

Add the plugin, put the tags in `<head>`, and render a sheet:

```rust
use autumn_plugin_univer::{
    Cell, Sheet, Spreadsheet, UniverPlugin, Workbook, WorkbookSnapshot,
    univer_script, univer_stylesheet,
};
use autumn_web::prelude::*;

#[get("/")]
async fn index() -> Markup {
    let workbook = Workbook::builder("budget", "Budget")
        .sheet(
            Sheet::new("q1", "Q1")
                .with_row(0, [Cell::text("Rent"), Cell::number(1200.0)])
                .with_cell(1, 1, Cell::formula("=B1*12")),
        )
        .build()
        .unwrap_or_else(|_| Workbook::empty("budget"));
    html! {
        html {
            head { (univer_stylesheet()) (univer_script()) }
            body { (Spreadsheet::new("budget").workbook(&workbook).save_url("/budget")) }
        }
    }
}

#[post("/budget")]
async fn save(WorkbookSnapshot(workbook): WorkbookSnapshot) -> &'static str {
    // Store `workbook` here. It is valid: see "Saving".
    let _ = workbook;
    "saved"
}

#[autumn_web::main]
async fn main() {
    autumn_web::app()
        .plugin(UniverPlugin::new())
        .routes(routes![index, save])
        .run()
        .await;
}
```

If CSRF is on, also put the token in `<head>`. `init.js` reads it:

```rust
meta name="csrf-token" content=(csrf.token());
```

## Demo

```sh
cargo run --example univer_demo
```

Open <http://127.0.0.1:3000>. Edit the budget, reload, open "Report", and
open the htmx scratch sheet.

## `Spreadsheet` options

| Method | Attribute | Default | Notes |
|---|---|---|---|
| `new(id)` | `id` | — | DOM id. `save_button` and `AutumnUniver` use it. |
| `.workbook(&wb)` | `<script type="application/json">` | empty sheet | Data in the page. The JSON is escaped against `</script>`. |
| `.load_url(url)` | `data-univer-load-url` | — | `GET` the workbook JSON instead. |
| `.save_url(url)` | `data-univer-save-url` | — | `POST` the snapshot JSON after edits. Same origin only. |
| `.autosave(d)` | `data-univer-autosave` | 1 s | Delay after the last edit (1 ms to 2³¹−1 ms). Needs `save_url`. |
| `.manual_save()` | `data-univer-autosave="0"` | — | Save only with `save_button` or `AutumnUniver.save`. Needs `save_url`. |
| `.height(css)` | `data-univer-height` | `480px` | A CSS length: `px`, `em`, `rem`, `vh`, `vw` or `%`. Other text is ignored. |
| `.label(text)` | `aria-label` | — | The accessible name of the region. |
| `.locale(l)` | `data-univer-locale` | `en-US` | See "Locales". |
| `.read_only(true)` | `data-univer-readonly` | `false` | Blocks typing and API edits. Never saves. The server must still authorize saves. |
| `.dark_mode(true)` | `data-univer-dark` | `false` | Univer dark theme. |
| `.header(false)` | `data-univer-header="false"` | shown | Menu and toolbar. |
| `.toolbar(false)` | `data-univer-toolbar="false"` | shown | |
| `.footer(false)` | `data-univer-footer="false"` | shown | Sheet tabs and zoom. |
| `.formula_bar(false)` | `data-univer-formula-bar="false"` | shown | |
| `.context_menu(false)` | `data-univer-context-menu="false"` | shown | |

`save_button("budget", "Save")` renders a button that saves the sheet with
id `budget`. It needs no JavaScript of your own.

## Saving

The browser POSTs `FWorkbook.save()` (Univer `IWorkbookData`) as JSON. The
request has `Content-Type: application/json` and the CSRF header from
`<meta name="csrf-token">` (`X-CSRF-Token`, or the meta `data-header`).

`WorkbookSnapshot` reads the body and checks it (Problem Details):

- `415`: the content type is not JSON.
- `413`: the body is over 8 MiB (`WorkbookSnapshot::MAX_BYTES`).
- `400`: the body is not JSON.
- `422`: the JSON is not a workbook, or it breaks an invariant or limit.

The invariants:

1. The workbook id is not empty.
2. `sheetOrder` has no duplicates.
3. `sheetOrder` and the keys of `sheets` hold the same ids.
4. Each sheet's `id` equals its key and is not empty.
5. Each cell is inside `rowCount` × `columnCount`.
6. Each number is finite.
7. Sizes are in `Limits` (default: 200 sheets, 1 048 576 × 16 384 cells
   per sheet, 1 000 000 set cells).
8. No `extra` map holds a typed field name (`id`, `v`, …).

For other limits, take `Json<Workbook>` and call
`workbook.validate_with(&Limits::default().with_max_cells(n))`.

The model types ids, names, sheet order, sizes, styles and cells. It keeps
every other field in `extra` (merges, row heights, resources, …). So
`workbook.to_json()` gives back the full snapshot. Store that text, or read
values with `sheet.value(row, col)` and `sheet.cells()`.
`sheet.to_rows()` gives dense rows; it refuses a grid over 1 000 000
entries, because one far cell (row 1 048 575) makes a huge grid.

## Events and JavaScript API

Each event fires on the sheet element and bubbles:

| Event | `detail` |
|---|---|
| `autumn-univer:ready` | `{ univerAPI, workbook }` |
| `autumn-univer:change` | `{ version }` |
| `autumn-univer:saving` | `{ version }` |
| `autumn-univer:saved` | `{ status, version, response }` |
| `autumn-univer:save-error` | `{ status, response }` or `{ error }` |
| `autumn-univer:error` | `{ error }` (mount failed) |

The element also has `data-univer-state`: `loading`, `ready`, `dirty`,
`saving`, `saved` or `error`. Use it in CSS.

`window.AutumnUniver` has `mount(el)`, `dispose(el)`, `save(el)`,
`scan(root)` and `get(el)` (gives `{ univerAPI, workbook }`). Put custom
code in an external file; the default CSP blocks inline scripts.

## htmx

`init.js` mounts sheets on `htmx:load` and disposes them on
`htmx:beforeCleanupElement`. A `MutationObserver` mounts sheets that enter
the DOM and disposes sheets that leave it in other ways. A sheet from the
htmx history cache mounts again.

Saves run one at a time, in order. Before dispose, `init.js` queues the
unsaved edits of an autosave sheet after the save in progress. When the
page unloads, it sends them with `keepalive` (64 KiB total). For a
manual-save sheet with unsaved edits, the browser asks before it leaves
the page.

## Locales

`en-US` is always in the bundle. Each other Univer locale is a Cargo
feature, about 0.7 MB each:

```toml
autumn-plugin-univer = { version = "0.1", features = ["locale-fr-fr"] }
```

Features: `locale-ar-sa`, `locale-ca-es`, `locale-de-de`, `locale-es-es`,
`locale-fa-ir`, `locale-fr-fr`, `locale-id-id`, `locale-it-it`,
`locale-ja-jp`, `locale-ko-kr`, `locale-pl-pl`, `locale-pt-br`,
`locale-ru-ru`, `locale-sk-sk`, `locale-vi-vn`, `locale-zh-cn`,
`locale-zh-hk`, `locale-zh-tw`, or `all-locales`. The `Locale` enum has a
variant only for the locales in your build. The browser loads the locale
file only when a sheet uses it.

`univer_script()` lists the compiled locales in a `<meta>` tag, so
`init.js` uses `en-US` for any other code and asks for no missing file.

`hyphenation` adds the rich-text hyphenation patterns (about 4.6 MB). Sheets
seldom need them. Without it, a rich-text cell with auto-hyphenation logs a
404 and shows no hyphens. The patterns come from TeX hyph-utf8, which uses
more than one license; check them before you ship this feature.

## How it works

- `vendor/` holds `package.json`, `package-lock.json` (exact pins) and
  `build.mjs`. esbuild makes one code-split ES module bundle in
  `assets/dist/`.
- `assets/manifest.json` records the package versions and the `sha384` of
  each file. A test compares the bytes with it. It is not served.
- `assets/dist/THIRD-PARTY-LICENSES.txt` holds the license texts of all 87
  bundled packages. It is served next to the bundle.
- `assets/init.js` and `assets/autumn-univer.css` are the plugin's own
  files.
- `UniverPlugin` installs all files as one Autumn `PluginAssets` bundle,
  `UNIVER_ASSETS`. Files serve under `/static/_plugins/univer/` at
  content-hashed URLs (`immutable`) and at plain URLs (`must-revalidate`).
  Lazy chunks load from the plain URLs.
- `univer_script()` and `univer_stylesheet()` emit hashed URLs with SRI.

## Security

- Render `Spreadsheet` markup only from your own templates. `init.js`
  mounts any `[data-univer]` element and saves with the CSRF token. If
  your app shows user HTML, strip `data-univer*` attributes from it.
- `init.js` accepts only same-origin load and save URLs, and saves refuse
  redirects.
- `read_only(true)` is a browser setting. Check who may save in the save
  handler.
- SRI covers `univer.js`, `init.js` and both stylesheets. Lazy chunks load
  from the same origin without SRI; their names hold a content hash.
- Univer adds `<style>` elements at run time. So `style-src` needs
  `'unsafe-inline'`, as in the Autumn default CSP. Autumn's nonce mode for
  styles (`csp_nonce`) blocks them.
- `WorkbookSnapshot` reads at most 8 MiB and checks sizes before your
  handler runs.

## Update Univer

```sh
cd vendor
npm install --save-exact @univerjs/presets@X @univerjs/preset-sheets-core@X
npm run build   # rewrites assets/dist, assets/manifest.json, src/bundle_files.rs
cd ..
cargo test --all-features
cargo test --features system-tests --test system
```

Then set `UNIVER_VERSION` in `src/assets.rs`. CI runs `npm ci && npm run
build` and fails if the output differs from the commit.

## Tests

```sh
cargo test                                             # unit, integration, doc
cargo test --features system-tests --test system       # Chromium
cargo test --features system-tests,all-locales --test system
```

The system tests use a real Chromium, the default Autumn CSP and CSRF.

## Limits

- Only the open-source sheets core preset is in. Univer Pro (XLSX import
  and export, collaboration, charts, pivot tables) is not.
- Each plugin release pins one Univer version.
- Univer 1.0 gives its cell editor a fixed DOM id. With more than one sheet
  on a page, `init.js` keeps the id only on the sheet that the user works
  in. Custom code that looks up Univer ids must expect this.
- The formula engine runs on the main thread. There is no web worker.
- Without JavaScript, the page shows a `<noscript>` notice.

## License

Apache-2.0. The bundled packages keep their own licenses; see
`assets/dist/THIRD-PARTY-LICENSES.txt`.
