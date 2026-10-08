//! Univer spreadsheets for Autumn, with Maud + htmx ergonomics.
//!
//! Add [`UniverPlugin`] to the app. Put [`univer_stylesheet()`] and
//! [`univer_script()`] in the page `<head>`. Render a [`Spreadsheet`]:
//!
//! ```rust,no_run
//! use autumn_plugin_univer::{
//!     Cell, Sheet, Spreadsheet, UniverPlugin, Workbook, univer_script, univer_stylesheet,
//! };
//! use autumn_web::prelude::*;
//!
//! #[get("/")]
//! async fn index() -> Markup {
//!     let workbook = Workbook::builder("budget", "Budget")
//!         .sheet(
//!             Sheet::new("q1", "Q1")
//!                 .with_row(0, [Cell::text("Rent"), Cell::number(1200.0)])
//!                 .with_cell(1, 1, Cell::formula("=B1*12")),
//!         )
//!         .build()
//!         .unwrap_or_else(|_| Workbook::empty("budget"));
//!     html! {
//!         html {
//!             head { (univer_stylesheet()) (univer_script()) }
//!             body {
//!                 (Spreadsheet::new("budget").workbook(&workbook).save_url("/budget"))
//!             }
//!         }
//!     }
//! }
//!
//! # async fn run() {
//! autumn_web::app()
//!     .plugin(UniverPlugin::new())
//!     .routes(routes![index])
//!     .run()
//!     .await;
//! # }
//! ```
//!
//! The plugin vendors [Univer](https://univer.ai) 1.0 (Apache-2.0) as one
//! code-split ES module bundle. No npm, no bundler and no CDN in the host
//! app. The bundle is the [`UNIVER_ASSETS`] Autumn plugin asset bundle. It
//! serves from memory under `/static/_plugins/univer/` with content-hashed,
//! immutable URLs and SRI hashes.
//!
//! Read a saved sheet back with the [`WorkbookSnapshot`] extractor. It
//! validates the data against the [`Workbook`] invariants and [`Limits`].
//!
//! # Limits
//!
//! - Only the open-source sheets core preset is in. Univer Pro features
//!   (XLSX import and export, collaboration, charts) are not.
//! - Each plugin release pins one Univer version (see
//!   [`UNIVER_VERSION`]).
//! - Without JavaScript, the page shows a `<noscript>` notice.

mod assets;
mod bundle_files;
mod extract;
#[cfg(test)]
mod init_js_tests;
mod locale;
mod plugin;
mod script;
mod spreadsheet;
mod workbook;

pub use assets::{ASSETS_NAMESPACE, UNIVER_ASSETS, UNIVER_VERSION};
pub use extract::WorkbookSnapshot;
pub use locale::{Locale, UnknownLocale};
pub use plugin::{PLUGIN_NAME, UniverPlugin};
pub use script::{univer_script, univer_stylesheet};
pub use spreadsheet::{Spreadsheet, save_button};
pub use workbook::{
    Cell, CellRef, CellType, CellValue, Limits, Sheet, SheetId, Workbook, WorkbookBuilder,
    WorkbookError, WorkbookId,
};
