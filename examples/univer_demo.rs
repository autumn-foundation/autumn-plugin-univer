//! Univer demo: a small Autumn app that uses `autumn-plugin-univer`.
//!
//! ```sh
//! cargo run --example univer_demo
//! ```
//!
//! Open <http://127.0.0.1:3000>, then:
//!
//! - Edit the budget. The sheet autosaves 1 s after the last edit.
//! - Reload the page. Your edits stay (the server keeps them in memory).
//! - Open "Report". Rust reads the saved workbook and renders a table. A
//!   read-only sheet shows the same data.
//! - Click "Open scratch sheet". htmx loads a second sheet. It saves only
//!   when you click its "Save" button.
//!
//! The app has no inline script and no inline style, so the default
//! Autumn CSP stays on. htmx comes from the framework.

use std::sync::{Mutex, OnceLock};

use autumn_plugin_univer::{
    Cell, CellValue, Sheet, Spreadsheet, UniverPlugin, Workbook, WorkbookSnapshot, save_button,
    univer_script, univer_stylesheet,
};
use autumn_web::prelude::*;
use autumn_web::security::CsrfToken;

/// The saved workbooks: the budget and the scratch sheet.
struct Store {
    budget: Workbook,
    scratch: Workbook,
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| {
        Mutex::new(Store {
            budget: seed_budget(),
            scratch: Workbook::empty("scratch"),
        })
    })
}

/// The first budget, made with the typed builder.
fn seed_budget() -> Workbook {
    let rows = [
        ("Rent", 1200.0),
        ("Food", 450.0),
        ("Transport", 120.0),
        ("Fun", 200.0),
    ];
    let mut sheet = Sheet::new("month", "Month")
        .with_row(0, [Cell::text("Item"), Cell::text("Cost")])
        .with_size(20, 6);
    let mut last = 1;
    for ((item, cost), row) in rows.into_iter().zip(1_u32..) {
        sheet.set_cell(row, 0, Cell::text(item));
        sheet.set_cell(row, 1, Cell::number(cost));
        last = row;
    }
    sheet.set_cell(last + 1, 0, Cell::text("Total").with_style("bold"));
    sheet.set_cell(
        last + 1,
        1,
        Cell::formula(format!("=SUM(B2:B{})", last + 1)).with_style("bold"),
    );
    Workbook::builder("budget", "Budget")
        .sheet(sheet)
        .style("bold", serde_json::json!({ "bl": 1 }))
        .build()
        .unwrap_or_else(|_| Workbook::empty("budget"))
}

#[autumn_web::main]
async fn main() {
    autumn_web::app()
        .plugin(UniverPlugin::new())
        .routes(routes![
            index,
            report,
            scratch,
            save_budget,
            save_scratch,
            budget_json
        ])
        .run()
        .await;
}

/// The page shell.
fn layout(title: &str, csrf: Option<&CsrfToken>, content: &Markup) -> Markup {
    html! {
        (maud::DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                // init.js reads the token for its save requests.
                @if let Some(token) = csrf {
                    meta name="csrf-token" content=(token.token());
                }
                title { (title) " · Univer demo" }
                (univer_stylesheet())
                script src=(asset_url("js/htmx.min.js")) defer {}
                (univer_script())
            }
            body {
                nav { a href="/" { "Budget" } " · " a href="/report" { "Report" } }
                main { (content) }
            }
        }
    }
}

#[get("/")]
async fn index(csrf: Option<CsrfToken>) -> Markup {
    let budget = store()
        .lock()
        .map_or_else(|_| seed_budget(), |s| s.budget.clone());
    let sheet = Spreadsheet::new("budget")
        .label("Monthly budget")
        .workbook(&budget)
        .save_url("/budget")
        .height("420px");
    layout(
        "Budget",
        csrf.as_ref(),
        &html! {
            h1 { "Monthly budget" }
            p { "Edit a cell. The sheet saves itself." }
            (sheet)
            p {
                button hx-get="/scratch" hx-target="#scratch-slot" hx-swap="innerHTML" {
                    "Open scratch sheet"
                }
            }
            div id="scratch-slot" {}
        },
    )
}

#[get("/scratch")]
async fn scratch() -> Markup {
    let workbook = store()
        .lock()
        .map_or_else(|_| Workbook::empty("scratch"), |s| s.scratch.clone());
    html! {
        h2 { "Scratch sheet" }
        (Spreadsheet::new("scratch")
            .label("Scratch sheet")
            .workbook(&workbook)
            .save_url("/scratch")
            .manual_save()
            .footer(false)
            .height("300px"))
        (save_button("scratch", "Save"))
    }
}

#[get("/report")]
async fn report(csrf: Option<CsrfToken>) -> Markup {
    let budget = store()
        .lock()
        .map_or_else(|_| seed_budget(), |s| s.budget.clone());
    // `to_rows` has a size cap; a client controls the saved data.
    let rows = budget
        .first_sheet()
        .and_then(|s| s.to_rows().ok())
        .unwrap_or_default();
    layout(
        "Report",
        csrf.as_ref(),
        &html! {
            h1 { "Budget report" }
            p { "Rust reads the saved workbook and renders this table." }
            table {
                @for row in &rows {
                    tr {
                        @for value in row {
                            td { (show(value.as_ref())) }
                        }
                    }
                }
            }
            h2 { "Read-only view" }
            (Spreadsheet::new("report").load_url("/budget.json").read_only(true).height("300px"))
        },
    )
}

fn show(value: Option<&CellValue>) -> String {
    match value {
        Some(CellValue::Text(t)) => t.clone(),
        Some(CellValue::Number(n)) => n.to_string(),
        Some(CellValue::Bool(b)) => b.to_string(),
        None => String::new(),
    }
}

#[get("/budget.json")]
async fn budget_json() -> Json<Workbook> {
    Json(
        store()
            .lock()
            .map_or_else(|_| seed_budget(), |s| s.budget.clone()),
    )
}

/// Refuses a snapshot of another workbook. (A real app also checks here
/// that the user may save.)
fn expect_id(workbook: &Workbook, id: &str) -> Result<(), AutumnError> {
    if workbook.id.as_str() == id {
        Ok(())
    } else {
        Err(AutumnError::unprocessable_msg(format!(
            "expected workbook `{id}`"
        )))
    }
}

#[post("/budget")]
async fn save_budget(WorkbookSnapshot(workbook): WorkbookSnapshot) -> AutumnResult<Json<bool>> {
    expect_id(&workbook, "budget")?;
    let ok = store().lock().map(|mut s| s.budget = workbook).is_ok();
    Ok(Json(ok))
}

#[post("/scratch")]
async fn save_scratch(WorkbookSnapshot(workbook): WorkbookSnapshot) -> AutumnResult<Json<bool>> {
    expect_id(&workbook, "scratch")?;
    let ok = store().lock().map(|mut s| s.scratch = workbook).is_ok();
    Ok(Json(ok))
}
