//! Chromium system tests: real browser, default Autumn CSP, CSRF on.
//!
//! Run: `cargo test --features system-tests --test system`.
//! They need Chromium (see `autumn_web::system_test` for the lookup).

// Test helpers outside `#[test]` functions may panic on setup errors.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::significant_drop_tightening
)]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use autumn_plugin_univer::{
    Cell, Sheet, Spreadsheet, UniverPlugin, Workbook, WorkbookSnapshot, save_button, univer_script,
    univer_stylesheet,
};
use autumn_web::config::AutumnConfig;
use autumn_web::extract::Path;
use autumn_web::prelude::*;
use autumn_web::security::CsrfToken;
use autumn_web::system_test::{Page, SystemTest, SystemTestRunner};
use autumn_web::test::TestApp;

// ---- Server ---------------------------------------------------------------

/// Snapshots the browser saved, by key.
fn saved() -> &'static Mutex<HashMap<String, Workbook>> {
    static SAVED: OnceLock<Mutex<HashMap<String, Workbook>>> = OnceLock::new();
    SAVED.get_or_init(Mutex::default)
}

fn seed(id: &str) -> Workbook {
    Workbook::builder(id, "Seed")
        .sheet(
            Sheet::new("s1", "Data")
                .with_cell(0, 0, Cell::number(42.0))
                .with_cell(0, 1, Cell::formula("=A1*2")),
        )
        .build()
        .expect("valid seed")
}

fn layout(csrf: Option<&CsrfToken>, body: &Markup) -> Markup {
    html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                @if let Some(token) = csrf {
                    meta name="csrf-token" content=(token.token());
                }
                (univer_stylesheet())
                script src=(asset_url("js/htmx.min.js")) defer {}
                (univer_script())
            }
            body { (body) }
        }
    }
}

#[get("/sheet/{key}")]
async fn sheet_page(Path(key): Path<String>, csrf: Option<CsrfToken>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .label("Test sheet")
        .workbook(&seed(&key))
        .save_url(format!("/save/{key}"))
        .autosave(Duration::from_millis(200));
    layout(csrf.as_ref(), &html! { (sheet) })
}

#[get("/no-csrf/{key}")]
async fn no_csrf_page(Path(key): Path<String>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .workbook(&seed(&key))
        .save_url(format!("/save/{key}"))
        .autosave(Duration::from_millis(200));
    layout(None, &html! { (sheet) })
}

#[get("/readonly")]
async fn readonly_page(csrf: Option<CsrfToken>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .workbook(&seed("ro"))
        .save_url("/save/ro")
        .autosave(Duration::from_millis(100))
        .read_only(true);
    layout(csrf.as_ref(), &html! { (sheet) })
}

#[get("/manual/{key}")]
async fn manual_page(Path(key): Path<String>, csrf: Option<CsrfToken>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .workbook(&seed(&key))
        .save_url(format!("/save/{key}"))
        .manual_save();
    layout(
        csrf.as_ref(),
        &html! { (sheet) (save_button("sheet", "Save")) },
    )
}

#[get("/loaded")]
async fn loaded_page(csrf: Option<CsrfToken>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .load_url("/data.json")
        .height("300px");
    layout(csrf.as_ref(), &html! { (sheet) })
}

#[get("/data.json")]
async fn data_json() -> Json<Workbook> {
    Json(seed("loaded"))
}

#[get("/two")]
async fn two_page(csrf: Option<CsrfToken>) -> Markup {
    layout(
        csrf.as_ref(),
        &html! {
            (Spreadsheet::new("one").workbook(&seed("one")))
            (Spreadsheet::new("two").workbook(&seed("two")).footer(false).toolbar(false))
        },
    )
}

#[get("/locale")]
async fn locale_page(csrf: Option<CsrfToken>) -> Markup {
    // A locale code this build may not hold: init.js falls back to en-US.
    layout(
        csrf.as_ref(),
        &html! {
            div id="sheet" class="autumn-univer" data-univer data-univer-locale="xx-XX" {
                div data-univer-mount {}
            }
        },
    )
}

#[cfg(feature = "locale-fr-fr")]
#[get("/french")]
async fn french_page(csrf: Option<CsrfToken>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .workbook(&seed("fr"))
        .locale(autumn_plugin_univer::Locale::FrFr);
    layout(csrf.as_ref(), &html! { (sheet) })
}

/// A known locale with no file in this build (no `locale-fr-fr`).
#[cfg(not(feature = "locale-fr-fr"))]
#[get("/missing-locale")]
async fn missing_locale_page(csrf: Option<CsrfToken>) -> Markup {
    layout(
        csrf.as_ref(),
        &html! {
            div id="sheet" class="autumn-univer" data-univer data-univer-locale="fr-FR" {
                div data-univer-mount {}
            }
        },
    )
}

#[get("/htmx")]
async fn htmx_page(csrf: Option<CsrfToken>) -> Markup {
    layout(
        csrf.as_ref(),
        &html! {
            button id="load" hx-get="/fragment" hx-target="#slot" { "Load" }
            button id="clear" hx-get="/empty" hx-target="#slot" { "Clear" }
            div id="slot" {}
        },
    )
}

#[get("/fragment")]
async fn fragment() -> Markup {
    Spreadsheet::new("sheet").workbook(&seed("frag")).render()
}

#[get("/empty")]
async fn empty() -> Markup {
    html! { p id="gone" { "Cleared" } }
}

/// Values of A2 that each slow save carried, in arrival order, plus the
/// highest number of saves in flight at once.
#[derive(Default)]
struct SlowLog {
    a2: Vec<String>,
    in_flight: usize,
    max_in_flight: usize,
}

fn slow_log() -> &'static Mutex<HashMap<String, SlowLog>> {
    static LOG: OnceLock<Mutex<HashMap<String, SlowLog>>> = OnceLock::new();
    LOG.get_or_init(Mutex::default)
}

/// Counts a slow save that starts.
fn begin_slow(key: &str) {
    let mut log = slow_log().lock().expect("lock");
    let entry = log.entry(key.to_owned()).or_default();
    entry.in_flight += 1;
    entry.max_in_flight = entry.max_in_flight.max(entry.in_flight);
}

/// Like `/save`, but takes 400 ms and logs each request.
#[post("/slow-save/{key}")]
async fn slow_save(Path(key): Path<String>, WorkbookSnapshot(wb): WorkbookSnapshot) -> Json<bool> {
    begin_slow(&key);
    tokio::time::sleep(Duration::from_millis(400)).await;
    let a2 = match wb.first_sheet().and_then(|s| s.value(1, 0)) {
        Some(autumn_plugin_univer::CellValue::Text(t)) => t.clone(),
        other => format!("{other:?}"),
    };
    let mut log = slow_log().lock().expect("lock");
    let entry = log.entry(key.clone()).or_default();
    entry.in_flight -= 1;
    entry.a2.push(a2);
    drop(log);
    saved().lock().expect("lock").insert(key, wb);
    Json(true)
}

#[get("/slow/{key}")]
async fn slow_page(Path(key): Path<String>, csrf: Option<CsrfToken>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .workbook(&seed(&key))
        .save_url(format!("/slow-save/{key}"))
        .autosave(Duration::from_millis(50));
    layout(
        csrf.as_ref(),
        &html! { (sheet) (save_button("sheet", "Save")) div id="other" {} },
    )
}

#[get("/bad-data")]
async fn bad_data_page(csrf: Option<CsrfToken>) -> Markup {
    layout(
        csrf.as_ref(),
        &html! {
            div id="sheet" class="autumn-univer" data-univer {
                script type="application/json" data-univer-data { "null" }
                div data-univer-mount {}
            }
        },
    )
}

#[get("/cross-origin")]
async fn cross_origin_page(csrf: Option<CsrfToken>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .workbook(&seed("xo"))
        .save_url("https://evil.example/steal")
        .autosave(Duration::from_millis(50));
    layout(csrf.as_ref(), &html! { (sheet) })
}

#[post("/save/{key}")]
async fn save(Path(key): Path<String>, WorkbookSnapshot(wb): WorkbookSnapshot) -> Json<bool> {
    saved().lock().expect("lock").insert(key, wb);
    Json(true)
}

/// Serves the app on an ephemeral port and attaches Chromium.
async fn start() -> SystemTestRunner {
    let mut config = AutumnConfig {
        profile: Some("test".into()),
        ..AutumnConfig::default()
    };
    config.security.csrf.enabled = true;
    #[allow(unused_mut)]
    let mut app = TestApp::new()
        .config(config)
        .plugin(UniverPlugin::new())
        .routes(routes![
            sheet_page,
            no_csrf_page,
            readonly_page,
            manual_page,
            loaded_page,
            data_json,
            two_page,
            locale_page,
            htmx_page,
            fragment,
            empty,
            save,
            slow_save,
            slow_page,
            bad_data_page,
            cross_origin_page
        ]);
    #[cfg(feature = "locale-fr-fr")]
    {
        app = app.routes(routes![french_page]);
    }
    #[cfg(not(feature = "locale-fr-fr"))]
    {
        app = app.routes(routes![missing_locale_page]);
    }
    let router = app.build().into_router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        autumn_web::reexports::axum::serve(listener, router)
            .await
            .expect("serve");
    });
    SystemTest::attach(format!("http://{addr}"))
        .await
        .expect("Chromium starts")
}

// ---- Browser helpers ------------------------------------------------------

/// Evaluates `js` and reads the result as `T`.
async fn eval<T: serde::de::DeserializeOwned>(page: &Page, js: &str) -> T {
    page.evaluate(js)
        .await
        .unwrap_or_else(|e| panic!("evaluate {js}: {e}"))
        .into_value()
        .unwrap_or_else(|e| panic!("decode {js}: {e}"))
}

/// Polls `js` until it is `true`, for up to 20 s.
async fn wait_for(page: &Page, js: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(result) = page.evaluate(js).await
            && result.into_value::<bool>().unwrap_or(false)
        {
            return;
        }
        if tokio::time::Instant::now() > deadline {
            let errors = page.console_errors();
            panic!("timed out waiting for `{js}`; console errors: {errors:?}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_state(page: &Page, id: &str, state: &str) {
    wait_for(
        page,
        &format!(
            "document.getElementById('{id}')?.getAttribute('data-univer-state') === '{state}'"
        ),
    )
    .await;
}

/// The value of `a1` in the active sheet of the spreadsheet `id`.
fn value_js(id: &str, a1: &str) -> String {
    format!(
        "AutumnUniver.get(document.getElementById('{id}')).workbook.getActiveSheet().getRange('{a1}').getValue()"
    )
}

/// Sets `a1` to the text `value` through the Univer facade.
fn set_js(id: &str, a1: &str, value: &str) -> String {
    format!(
        "(async () => {{ try {{ return await AutumnUniver.get(document.getElementById('{id}')).workbook.getActiveSheet().getRange('{a1}').setValue('{value}') !== undefined; }} catch (e) {{ return false; }} }})()"
    )
}

fn saved_cell(key: &str, row: u32, col: u32) -> Option<autumn_plugin_univer::CellValue> {
    saved()
        .lock()
        .expect("lock")
        .get(key)
        .and_then(|wb| wb.first_sheet().and_then(|s| s.value(row, col).cloned()))
}

// ---- Tests ----------------------------------------------------------------

#[tokio::test]
async fn mounts_under_the_default_csp_with_no_console_errors() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/sheet/mount").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    // The formula engine runs after mount: B1 = A1 * 2.
    wait_for(&page, &format!("{} === 84", value_js("sheet", "B1"))).await;
    let canvases: u32 = eval(&page, "document.querySelectorAll('#sheet canvas').length").await;
    assert!(canvases > 0, "Univer draws on a canvas");
    page.expect_no_console_errors()
        .await
        .expect("clean console");
    // No save without an edit (autosave is 200 ms: wait well past it).
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(
        saved().lock().expect("lock").get("mount").is_none(),
        "no spurious save"
    );
}

#[tokio::test]
async fn an_edit_autosaves_with_the_csrf_token() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/sheet/edit").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let set: bool = eval(&page, &set_js("sheet", "A2", "hello")).await;
    assert!(set, "setValue works");
    wait_state(&page, "sheet", "saved").await;
    assert_eq!(saved_cell("edit", 1, 0), Some("hello".into()));
    assert_eq!(
        saved_cell("edit", 0, 0),
        Some(42.0.into()),
        "old data stays"
    );
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

#[tokio::test]
async fn a_save_without_the_csrf_token_reports_an_error() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/no-csrf/nocsrf").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    eval::<bool>(
        &page,
        "(() => { window.__errors = []; document.addEventListener('autumn-univer:save-error', e => window.__errors.push(e.detail.status)); return true; })()",
    )
    .await;
    let _: bool = eval(&page, &set_js("sheet", "A2", "x")).await;
    wait_state(&page, "sheet", "error").await;
    let statuses: Vec<u16> = eval(&page, "window.__errors").await;
    assert_eq!(statuses, [403], "CSRF layer rejects the POST");
    assert!(saved().lock().expect("lock").get("nocsrf").is_none());
}

#[tokio::test]
async fn read_only_sheets_block_edits_and_never_save() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/readonly").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    // Univer's setValue returns the range even when the permission mode
    // refuses the edit, so the check is on the value below.
    let _: bool = eval(&page, &set_js("sheet", "A1", "changed")).await;
    // Autosave is 100 ms: wait well past it.
    tokio::time::sleep(Duration::from_millis(1000)).await;
    let a1: f64 = eval(&page, &value_js("sheet", "A1")).await;
    assert!((a1 - 42.0).abs() < f64::EPSILON, "A1 unchanged: {a1}");
    let state: String = eval(
        &page,
        "document.getElementById('sheet').getAttribute('data-univer-state')",
    )
    .await;
    assert_eq!(state, "ready");
    assert!(saved().lock().expect("lock").get("ro").is_none());
}

#[tokio::test]
async fn the_save_button_saves_a_manual_sheet() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/manual/manual").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let _: bool = eval(&page, &set_js("sheet", "C3", "typed")).await;
    wait_state(&page, "sheet", "dirty").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        saved().lock().expect("lock").get("manual").is_none(),
        "no autosave"
    );
    page.click("Save").await.expect("click");
    wait_state(&page, "sheet", "saved").await;
    assert_eq!(saved_cell("manual", 2, 2), Some("typed".into()));
}

#[tokio::test]
async fn load_url_fetches_the_workbook() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/loaded").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let a1: f64 = eval(&page, &value_js("sheet", "A1")).await;
    assert!((a1 - 42.0).abs() < f64::EPSILON);
    let height: String = eval(&page, "document.getElementById('sheet').style.height").await;
    assert_eq!(height, "300px", "init.js sets the height through CSSOM");
}

#[tokio::test]
async fn two_sheets_mount_on_one_page() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/two").await.expect("visit");
    wait_state(&page, "one", "ready").await;
    wait_state(&page, "two", "ready").await;
    let ids: Vec<String> = eval(
        &page,
        "['one','two'].map(id => AutumnUniver.get(document.getElementById(id)).workbook.getId())",
    )
    .await;
    assert_eq!(ids, ["one", "two"]);
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

#[tokio::test]
async fn only_the_active_sheet_keeps_the_editor_ids() {
    // Univer finds its cell editor by a fixed DOM id. With two sheets, the
    // id must point into the sheet that the user works in.
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/two").await.expect("visit");
    wait_state(&page, "one", "ready").await;
    wait_state(&page, "two", "ready").await;
    let owner = "(() => { const n = document.querySelectorAll('[id=\"__editor___INTERNAL_EDITOR__DOCS_NORMAL\"]'); return n.length === 1 ? n[0].closest('[data-univer]').id : 'count=' + n.length; })()";
    let press = |id: &str| {
        format!(
            "(document.querySelector('#{id} canvas').dispatchEvent(new PointerEvent('pointerdown', {{ bubbles: true }})), true)"
        )
    };
    for id in ["two", "one", "two"] {
        let _: bool = eval(&page, &press(id)).await;
        let got: String = eval(&page, owner).await;
        assert_eq!(got, id, "editor id belongs to the active sheet");
    }
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

#[tokio::test]
async fn unknown_locales_fall_back_to_en_us() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/locale").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let locale: String = eval(
        &page,
        "AutumnUniver.get(document.getElementById('sheet')).univerAPI.getCurrentLocale()",
    )
    .await;
    assert_eq!(locale, "enUS");
    page.expect_no_console_errors()
        .await
        .expect("a warning, not an error");
}

#[cfg(feature = "locale-fr-fr")]
#[tokio::test]
async fn a_bundled_locale_loads_lazily() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/french").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let locale: String = eval(
        &page,
        "AutumnUniver.get(document.getElementById('sheet')).univerAPI.getCurrentLocale()",
    )
    .await;
    assert_eq!(locale, "frFR");
    let loaded: bool = eval(
        &page,
        "performance.getEntriesByType('resource').some(e => /chunks\\/fr-FR-/.test(e.name))",
    )
    .await;
    assert!(loaded, "the fr-FR chunk loads on demand");
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

#[tokio::test]
async fn htmx_swaps_mount_and_cleanup_disposes() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/htmx").await.expect("visit");
    page.click("#load").await.expect("click load");
    wait_state(&page, "sheet", "ready").await;
    eval::<bool>(
        &page,
        "(window.__el = document.getElementById('sheet'), true)",
    )
    .await;
    page.click("#clear").await.expect("click clear");
    wait_for(&page, "document.getElementById('gone') !== null").await;
    wait_for(&page, "AutumnUniver.get(window.__el) === undefined").await;
    let marker: bool = eval(&page, "window.__el.hasAttribute('data-univer-init')").await;
    assert!(!marker, "dispose clears the mount marker");
    // Load again: a fresh instance mounts.
    page.click("#load").await.expect("click load again");
    wait_state(&page, "sheet", "ready").await;
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

fn slow(key: &str) -> (Vec<String>, usize) {
    slow_log()
        .lock()
        .expect("lock")
        .get(key)
        .map(|l| (l.a2.clone(), l.max_in_flight))
        .unwrap_or_default()
}

#[tokio::test]
async fn edits_during_a_save_and_before_dispose_all_arrive_in_order() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/slow/race").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let _: bool = eval(&page, &set_js("sheet", "A2", "first")).await;
    wait_state(&page, "sheet", "saving").await;
    let _: bool = eval(&page, &set_js("sheet", "A2", "second")).await;
    // Remove the sheet while the first save is in flight.
    let _: bool = eval(&page, "(document.getElementById('sheet').remove(), true)").await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while slow("race").0.len() < 2 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "saves: {:?}",
            slow("race")
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let (values, max_in_flight) = slow("race");
    assert_eq!(values, ["first", "second"], "no edit is lost");
    assert_eq!(max_in_flight, 1, "one save at a time");
    assert_eq!(
        saved_cell("race", 1, 0),
        Some("second".into()),
        "newest data wins"
    );
}

#[tokio::test]
async fn rapid_manual_saves_run_one_at_a_time() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/slow/rapid").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let _: bool = eval(&page, &set_js("sheet", "A2", "v1")).await;
    wait_state(&page, "sheet", "saving").await;
    let _: bool = eval(&page, &set_js("sheet", "A2", "v2")).await;
    for _ in 0..3 {
        page.click("Save").await.expect("click");
    }
    wait_state(&page, "sheet", "saved").await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    let (values, max_in_flight) = slow("rapid");
    assert_eq!(max_in_flight, 1, "saves never overlap: {values:?}");
    assert_eq!(values.last().map(String::as_str), Some("v2"), "{values:?}");
    assert!(
        values.len() <= 3,
        "queued clicks share one save: {values:?}"
    );
    assert_eq!(saved_cell("rapid", 1, 0), Some("v2".into()));
}

#[tokio::test]
async fn a_copy_of_mounted_markup_mounts_again() {
    // htmx history restores a copy of the live DOM, markers and all.
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/sheet/copy").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let _: bool = eval(
        &page,
        "(() => { const el = document.getElementById('sheet'); const html = el.outerHTML; el.remove(); document.body.insertAdjacentHTML('beforeend', html); return true; })()",
    )
    .await;
    wait_for(
        &page,
        "AutumnUniver.get(document.getElementById('sheet')) !== undefined",
    )
    .await;
    wait_state(&page, "sheet", "ready").await;
    let canvases: u32 = eval(&page, "document.querySelectorAll('#sheet canvas').length").await;
    assert!(
        canvases > 0 && canvases < 10,
        "one live instance: {canvases} canvases"
    );
}

#[tokio::test]
async fn a_sheet_moved_later_mounts_again() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/sheet/moved").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let _: bool = eval(
        &page,
        "(() => { const el = document.getElementById('sheet'); el.remove(); setTimeout(() => document.body.append(el), 50); return true; })()",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    wait_state(&page, "sheet", "ready").await;
    wait_for(
        &page,
        "AutumnUniver.get(document.getElementById('sheet')) !== undefined",
    )
    .await;
}

#[tokio::test]
async fn dispose_during_loading_stops_the_mount() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/htmx").await.expect("visit");
    page.click("#load").await.expect("click load");
    // Dispose as soon as the mount starts.
    wait_for(
        &page,
        "document.getElementById('sheet')?.getAttribute('data-univer-state') === 'loading'",
    )
    .await;
    let _: bool = eval(
        &page,
        "(window.__el = document.getElementById('sheet'), AutumnUniver.dispose(window.__el), true)",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let cleared: bool = eval(
        &page,
        "window.__el.getAttribute('data-univer-state') === null",
    )
    .await;
    assert!(cleared, "no ready after dispose");
    let gone: bool = eval(&page, "AutumnUniver.get(window.__el) === undefined").await;
    assert!(gone);
    // A new mount works.
    let _: bool = eval(&page, "(AutumnUniver.mount(window.__el), true)").await;
    wait_state(&page, "sheet", "ready").await;
    let canvases: u32 = eval(&page, "document.querySelectorAll('#sheet canvas').length").await;
    assert!(
        canvases > 0 && canvases < 10,
        "one live instance: {canvases} canvases"
    );
}

#[tokio::test]
async fn bad_data_fails_cleanly() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/bad-data").await.expect("visit");
    wait_state(&page, "sheet", "error").await;
    let children: u32 = eval(
        &page,
        "document.querySelector('#sheet [data-univer-mount]').children.length",
    )
    .await;
    assert_eq!(children, 0, "the failed instance is freed");
}

#[tokio::test]
async fn a_cross_origin_save_url_is_refused() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/cross-origin").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let _: bool = eval(&page, &set_js("sheet", "A2", "secret")).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let state: String = eval(
        &page,
        "document.getElementById('sheet').getAttribute('data-univer-state')",
    )
    .await;
    assert_eq!(state, "dirty", "no save request");
    let errors = page.console_errors();
    assert!(
        errors.iter().any(|e| e.contains("not a same-origin URL")),
        "{errors:?}"
    );
}

#[cfg(not(feature = "locale-fr-fr"))]
#[tokio::test]
async fn a_locale_missing_from_the_build_is_not_fetched() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/missing-locale").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let locale: String = eval(
        &page,
        "AutumnUniver.get(document.getElementById('sheet')).univerAPI.getCurrentLocale()",
    )
    .await;
    assert_eq!(locale, "enUS");
    let fetched: bool = eval(
        &page,
        "performance.getEntriesByType('resource').some(e => /chunks\\/fr-FR-/.test(e.name))",
    )
    .await;
    assert!(!fetched, "no request for a missing locale file");
    page.expect_no_console_errors().await.expect("no 404");
}

#[tokio::test]
async fn a_remount_during_a_mount_renders() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/htmx").await.expect("visit");
    // Dispose and mount again after createUniver, before ready.
    let _: bool = eval(
        &page,
        "(async () => { const slot = document.getElementById('slot'); await new Promise((res) => { const mo = new MutationObserver(() => { const el = document.getElementById('sheet'); if (el && el.querySelector('canvas') && el.getAttribute('data-univer-state') === 'loading') { mo.disconnect(); AutumnUniver.dispose(el); AutumnUniver.mount(el); res(); } }); mo.observe(slot, { childList: true, subtree: true }); document.getElementById('load').click(); }); return true; })()",
    )
    .await;
    wait_state(&page, "sheet", "ready").await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let canvases: u32 = eval(&page, "document.querySelectorAll('#sheet canvas').length").await;
    assert!(canvases > 0, "the new instance keeps its DOM");
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

#[tokio::test]
async fn a_reattached_sheet_keeps_its_unsaved_edits() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/manual/reattach").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let _: bool = eval(&page, &set_js("sheet", "A2", "typed")).await;
    wait_state(&page, "sheet", "dirty").await;
    let _: bool = eval(
        &page,
        "(() => { const el = document.getElementById('sheet'); el.remove(); setTimeout(() => document.body.prepend(el), 100); return true; })()",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    wait_for(
        &page,
        "AutumnUniver.get(document.getElementById('sheet')) !== undefined",
    )
    .await;
    let a2: String = eval(&page, &value_js("sheet", "A2")).await;
    assert_eq!(a2, "typed", "the re-mount uses the last snapshot");
    wait_state(&page, "sheet", "dirty").await;
}

#[tokio::test]
async fn dispose_sends_no_duplicate_of_an_in_flight_save() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/slow/dup").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let _: bool = eval(&page, &set_js("sheet", "A2", "only")).await;
    wait_state(&page, "sheet", "saving").await;
    let _: bool = eval(
        &page,
        "(AutumnUniver.dispose(document.getElementById('sheet')), true)",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(slow("dup").0, ["only"], "one request per version");
}

#[tokio::test]
async fn unload_flushes_a_save_queued_by_dispose() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/slow/unload").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    let _: bool = eval(&page, &set_js("sheet", "A2", "first")).await;
    wait_state(&page, "sheet", "saving").await;
    let _: bool = eval(&page, &set_js("sheet", "A2", "second")).await;
    let _: bool = eval(
        &page,
        "(AutumnUniver.dispose(document.getElementById('sheet')), true)",
    )
    .await;
    page.visit("/empty").await.expect("leave the page");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !slow("unload").0.contains(&"second".to_owned()) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "saves: {:?}",
            slow("unload")
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
