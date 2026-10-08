//! [`WorkbookSnapshot`]: reads and validates a saved workbook.

use autumn_web::AutumnError;
use autumn_web::extract::Json;
use autumn_web::reexports::axum::extract::{FromRequest, Request};

use crate::workbook::Workbook;

/// A request body that holds a valid [`Workbook`].
///
/// A [`Spreadsheet`](crate::Spreadsheet) with a save URL POSTs its snapshot
/// as JSON. Take it in the handler with this extractor:
///
/// ```rust,no_run
/// use autumn_plugin_univer::WorkbookSnapshot;
/// use autumn_web::prelude::*;
///
/// #[post("/budget")]
/// async fn save(WorkbookSnapshot(workbook): WorkbookSnapshot) -> &'static str {
///     // Store `workbook` (or `workbook.to_json()`) here.
///     let _ = workbook;
///     "saved"
/// }
/// ```
///
/// Bad JSON gives `400` (or `415` for a wrong content type). JSON that
/// breaks a [`Workbook`] invariant or the default [`Limits`](crate::Limits)
/// gives `422`. Both use the Autumn Problem Details format.
///
/// The axum default body limit is 2 MB. For larger sheets, add a
/// `DefaultBodyLimit` layer to the route.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkbookSnapshot(pub Workbook);

impl WorkbookSnapshot {
    /// The workbook.
    #[must_use]
    pub fn into_inner(self) -> Workbook {
        self.0
    }
}

impl<S> FromRequest<S> for WorkbookSnapshot
where
    S: Send + Sync,
{
    type Rejection = AutumnError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(workbook) = Json::<Workbook>::from_request(req, state).await?;
        workbook.validate().map_err(AutumnError::unprocessable)?;
        Ok(Self(workbook))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autumn_web::prelude::*;
    use autumn_web::test::TestApp;
    use serde_json::json;

    #[post("/save")]
    async fn save(WorkbookSnapshot(wb): WorkbookSnapshot) -> String {
        format!("{}:{}", wb.id, wb.sheets.len())
    }

    fn client() -> autumn_web::test::TestClient {
        TestApp::new().routes(routes![save]).build()
    }

    async fn post(body: &serde_json::Value) -> autumn_web::test::TestResponse {
        client()
            .post("/save")
            .header("content-type", "application/json")
            .body(body.to_string())
            .send()
            .await
    }

    #[tokio::test]
    async fn accepts_a_valid_snapshot() {
        let body = json!({
            "id": "wb", "sheetOrder": ["s"],
            "sheets": { "s": { "id": "s", "cellData": { "0": { "0": { "v": 1 } } } } }
        });
        let response = post(&body).await;
        response.assert_ok();
        assert_eq!(response.text(), "wb:1");
    }

    #[tokio::test]
    async fn rejects_broken_invariants_with_422() {
        let body = json!({
            "id": "wb", "sheetOrder": ["s", "ghost"],
            "sheets": { "s": { "id": "s" } }
        });
        let response = post(&body).await;
        response.assert_status(422);
        assert!(response.text().contains("ghost"), "{}", response.text());
    }

    #[tokio::test]
    async fn rejects_out_of_bounds_cells_with_422() {
        let body = json!({
            "id": "wb", "sheetOrder": ["s"],
            "sheets": { "s": { "id": "s", "rowCount": 1, "columnCount": 1,
                "cellData": { "3": { "0": { "v": 1 } } } } }
        });
        post(&body).await.assert_status(422);
    }

    #[tokio::test]
    async fn rejects_bad_json_with_400() {
        client()
            .post("/save")
            .header("content-type", "application/json")
            .body("{ nope")
            .send()
            .await
            .assert_status(400);
    }

    #[tokio::test]
    async fn rejects_a_wrong_shape_with_a_client_error() {
        let response = post(&json!({ "sheets": [] })).await;
        assert!(
            (400..500).contains(&response.status.as_u16()),
            "{}",
            response.status
        );
    }

    #[tokio::test]
    async fn rejects_a_missing_content_type() {
        let response = client().post("/save").body("{}").send().await;
        response.assert_status(415);
    }

    #[test]
    fn into_inner_returns_the_workbook() {
        let wb = Workbook::empty("w");
        assert_eq!(WorkbookSnapshot(wb.clone()).into_inner(), wb);
    }
}
