# Plan: autumn-plugin-univer 0.1.0

Status: done. Target: autumn-web 0.8.0, Univer 1.0.3.

## Goal

Give Autumn apps a spreadsheet. The server renders the page. Univer runs
in the browser. The app keeps the data. Use no npm, no bundler and no CDN
in the host app.

## Source issue

The repository has no issue. This plan is the source of the acceptance
criteria (AC).

## Brainstorming

Ideas, with no filter:

1. Serve Univer through the Autumn `plugin_assets` seam, as in
   `autumn-plugin-motion`.
2. Make one esbuild bundle from the npm packages. Put the output in the
   crate.
3. Load Univer UMD files from a CDN.
4. A typed Rust builder that renders a mount `<div>`.
5. A Rust workbook model for seeding data and reading saves.
6. Autosave: POST the snapshot to an app URL after edits.
7. Read-only mode for reports.
8. htmx: mount on swap, dispose on cleanup.
9. Load locale strings on demand.
10. Real-time collaboration through `autumn_web::collab`.
11. Server-side formula engine.
12. Import and export of XLSX files.
13. Dark mode.
14. A browser test with headless Chromium.

Selected for 0.1.0: 1, 2, 4, 5, 6, 7, 8, 9, 13, 14.
Deferred: 3 (breaks the default CSP and offline use), 10, 11, 12 (each
is a large feature; Univer gives XLSX and collaboration only in paid
plugins).

## Reverse brainstorming

Question: "How can we make this plugin fail?" Each answer gives a
counter-measure.

| How to fail | Counter-measure |
|---|---|
| Inline `<script>` code. The default CSP blocks it. | External module scripts only. JSON data goes in `type="application/json"` blocks, which the browser does not run. |
| Inline `style=` attributes. Nonce-mode CSP blocks them. | Height goes in `data-univer-height`. `init.js` sets it through CSSOM. |
| Put `</script>` in a cell value. The page breaks (XSS). | Escape `<`, `>`, `&`, U+2028 and U+2029 in the JSON block. Test it. |
| Drop unknown snapshot fields on save. Styles and merges get lost. | The model keeps unknown fields (`#[serde(flatten)]`). A round-trip test uses a real Univer snapshot. |
| Accept any JSON on save. A client sends 10⁹ cells. | `WorkbookSnapshot` validates structure and limits. It returns 422. |
| Forget CSRF on save. Autumn rejects the POST. | `init.js` sends the token from `<meta name="csrf-token">`, as the Autumn htmx helper does. |
| Mount twice after an htmx swap. | `init.js` marks mounted nodes with `data-univer-init`. |
| Leak Univer instances on swap. | Dispose on `htmx:beforeCleanupElement`. |
| Vendor bytes drift with no record. | `assets/manifest.json` pins versions and hashes. A test checks the hashes. CI rebuilds the bundle and compares. |
| Load 11 MB on every page. | Code-split ESM. Hyphenation data and locales load on demand. |
| Test strings only, never a real browser. | A Chromium system test mounts, edits, saves and disposes. |

## Six thinking hats

- **White (facts).** Univer 1.0.3 is Apache-2.0. Presets ship ES and CJS
  only; no UMD. The esbuild ESM bundle is 6.2 MB (main) plus lazy chunks.
  A smoke test showed two instances work under the Autumn default CSP.
  autumn-web 0.8 gives `PluginAssets` with SRI and immutable caching.
  `SystemTest::attach` drives Chromium.
- **Red (feelings).** A spreadsheet in a server-rendered app must feel
  like one line of Rust. Users fear lost edits.
- **Black (risks).** Bundle size. Univer API changes between versions.
  Multiple instances share global CSS. Snapshots above the 2 MB axum body
  limit. No Verus in this environment.
- **Yellow (benefits).** No JS toolchain for the app author. SRI on every
  file. Typed data in and out. Works with htmx partials.
- **Green (ideas).** Lazy locales. `autumn-univer:*` DOM events as the
  extension point. `window.AutumnUniver` for custom code.
- **Blue (process).** SPEC → RED → GREEN → REFACTOR per module:
  assets, plugin, script tags, workbook model, spreadsheet builder,
  `init.js`, system test. Then review with agents and fix findings.

## Acceptance criteria

1. **AC1** `UniverPlugin` installs one `PluginAssets` bundle (namespace
   `univer`). Files serve under `/static/_plugins/univer/` with hashed,
   immutable URLs, SRI, ETag/304. The plugin passes Autumn conformance.
2. **AC2** `univer_head()` renders the stylesheet and module-script tags
   with SRI, in the correct order.
3. **AC3** A typed `Spreadsheet` builder renders the mount markup:
   id, height, locale, read-only, dark mode, UI toggles, inline data or
   load URL, save URL and autosave delay.
4. **AC4** Inline workbook JSON is safe against `</script>` injection.
5. **AC5** A Rust `Workbook` model seeds data (builder, rows) and reads
   saved snapshots with no field loss.
6. **AC6** Workbook invariants are specified and enforced: unique and
   matching sheet ids, cells in bounds, size limits.
7. **AC7** `WorkbookSnapshot` extractor validates POSTed snapshots and
   returns 422 (Problem Details) on bad input.
8. **AC8** In a real browser, under the default Autumn CSP, the sheet
   mounts with no console errors.
9. **AC9** An edit triggers a debounced POST of the snapshot with the
   CSRF header. Events `ready`, `change`, `saved`, `save-error` fire.
10. **AC10** Read-only mode blocks edits.
11. **AC11** htmx-swapped sheets mount; removed sheets dispose.
12. **AC12** All 19 Univer locales load on demand.
13. **AC13** The vendored bundle is reproducible (pinned lockfile,
    build script, manifest with hashes, CI check) and license notices
    ship.
14. **AC14** A runnable example app shows seed, edit, save and reload.
15. **AC15** README, ADRs and doc comments use short ASD-STE100 text.
16. **AC16** CI runs fmt, clippy (pedantic, nursery), tests, system
    tests and coverage.

## Invariant spec (for AC6)

Verus is not available in this environment (GitHub release downloads are
blocked). The spec is below. `Workbook::validate` enforces it.
Property tests check it.

```text
valid(w) ⇔
  w.id ≠ "" ∧
  |w.sheet_order| = |w.sheets| ∧
  ∀ i ≠ j. w.sheet_order[i] ≠ w.sheet_order[j] ∧
  ∀ id ∈ w.sheet_order. id ∈ keys(w.sheets) ∧ w.sheets[id].id = id ∧
  ∀ s ∈ w.sheets, (r, c) ∈ keys(s.cells). r < s.row_count ∧ c < s.column_count ∧
  |w.sheets| ≤ limits.max_sheets ∧ Σ |s.cells| ≤ limits.max_cells ∧
  ∀ s. s.row_count ≤ limits.max_rows ∧ s.column_count ≤ limits.max_columns

Post(builder.build()) = Ok(w) ⇒ valid(w)
Post(validate(w)) = Ok ⇔ valid(w)
```

## Out of scope

XLSX import and export, real-time collaboration, Univer Pro plugins,
web-worker formula engine, docs and slides presets.
