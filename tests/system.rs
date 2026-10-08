//! Chromium system tests: real browser, default Autumn CSP, CSRF on.
//!
//! Run: `cargo test --features system-tests --test system`.
//! They need Chromium (see `autumn_web::system_test` for the lookup).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use autumn_plugin_univer::{
    Cell, Sheet, Spreadsheet, UniverPlugin, Workbook, WorkbookSnapshot, save_button,
    univer_script, univer_stylesheet,
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

fn layout(csrf: Option<&CsrfToken>, body: Markup) -> Markup {
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
    layout(csrf.as_ref(), html! { (sheet) })
}

#[get("/no-csrf/{key}")]
async fn no_csrf_page(Path(key): Path<String>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .workbook(&seed(&key))
        .save_url(format!("/save/{key}"))
        .autosave(Duration::from_millis(200));
    layout(None, html! { (sheet) })
}

#[get("/readonly")]
async fn readonly_page(csrf: Option<CsrfToken>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .workbook(&seed("ro"))
        .save_url("/save/ro")
        .autosave(Duration::from_millis(100))
        .read_only(true);
    layout(csrf.as_ref(), html! { (sheet) })
}

#[get("/manual/{key}")]
async fn manual_page(Path(key): Path<String>, csrf: Option<CsrfToken>) -> Markup {
    let sheet = Spreadsheet::new("sheet")
        .workbook(&seed(&key))
        .save_url(format!("/save/{key}"))
        .manual_save();
    layout(csrf.as_ref(), html! { (sheet) (save_button("sheet", "Save")) })
}

#[get("/loaded")]
async fn loaded_page(csrf: Option<CsrfToken>) -> Markup {
    let sheet = Spreadsheet::new("sheet").load_url("/data.json").height("300px");
    layout(csrf.as_ref(), html! { (sheet) })
}

#[get("/data.json")]
async fn data_json() -> Json<Workbook> {
    Json(seed("loaded"))
}

#[get("/two")]
async fn two_page(csrf: Option<CsrfToken>) -> Markup {
    layout(
        csrf.as_ref(),
        html! {
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
        html! {
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
    layout(csrf.as_ref(), html! { (sheet) })
}

#[get("/htmx")]
async fn htmx_page(csrf: Option<CsrfToken>) -> Markup {
    layout(
        csrf.as_ref(),
        html! {
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

#[post("/save/{key}")]
async fn save(Path(key): Path<String>, WorkbookSnapshot(wb): WorkbookSnapshot) -> Json<bool> {
    saved().lock().expect("lock").insert(key, wb);
    Json(true)
}

/// Serves the app on an ephemeral port and attaches Chromium.
async fn start() -> SystemTestRunner {
    let mut config = AutumnConfig::default();
    config.profile = Some("test".into());
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
            save
        ]);
    #[cfg(feature = "locale-fr-fr")]
    {
        app = app.routes(routes![french_page]);
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
        if let Ok(result) = page.evaluate(js).await {
            if result.into_value::<bool>().unwrap_or(false) {
                return;
            }
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
        &format!("document.getElementById('{id}')?.getAttribute('data-univer-state') === '{state}'"),
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
    page.expect_no_console_errors().await.expect("clean console");
    // No save without an edit.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(saved().lock().expect("lock").get("mount").is_none(), "no spurious save");
}

#[tokio::test]
async fn the_page_carries_the_default_csp() {
    let client = TestApp::new().plugin(UniverPlugin::new()).routes(routes![sheet_page]).build();
    let response = client.get("/sheet/csp").send().await;
    let csp = response.header("content-security-policy").expect("CSP header");
    assert!(csp.contains("script-src 'self'"), "{csp}");
    assert!(!csp.contains("unsafe-eval"), "{csp}");
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
    assert_eq!(saved_cell("edit", 0, 0), Some(42.0.into()), "old data stays");
    page.expect_no_console_errors().await.expect("clean console");
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
    let _: bool = eval(&page, &set_js("sheet", "A1", "changed")).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let a1: f64 = eval(&page, &value_js("sheet", "A1")).await;
    assert!((a1 - 42.0).abs() < f64::EPSILON, "A1 unchanged: {a1}");
    let state: String = eval(&page, "document.getElementById('sheet').getAttribute('data-univer-state')").await;
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
    assert!(saved().lock().expect("lock").get("manual").is_none(), "no autosave");
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
    page.expect_no_console_errors().await.expect("clean console");
}

#[tokio::test]
async fn unknown_locales_fall_back_to_en_us() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/locale").await.expect("visit");
    wait_state(&page, "sheet", "ready").await;
    page.expect_no_console_errors().await.expect("a warning, not an error");
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
    page.expect_no_console_errors().await.expect("clean console");
}

#[tokio::test]
async fn htmx_swaps_mount_and_cleanup_disposes() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/htmx").await.expect("visit");
    page.click("#load").await.expect("click load");
    wait_state(&page, "sheet", "ready").await;
    eval::<bool>(&page, "(window.__el = document.getElementById('sheet'), true)").await;
    page.click("#clear").await.expect("click clear");
    wait_for(&page, "document.getElementById('gone') !== null").await;
    wait_for(&page, "AutumnUniver.get(window.__el) === undefined").await;
    let marker: bool = eval(&page, "window.__el.hasAttribute('data-univer-init')").await;
    assert!(!marker, "dispose clears the mount marker");
    // Load again: a fresh instance mounts.
    page.click("#load").await.expect("click load again");
    wait_state(&page, "sheet", "ready").await;
    page.expect_no_console_errors().await.expect("clean console");
}
