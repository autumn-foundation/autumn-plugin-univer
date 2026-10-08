//! [`WorkbookSnapshot`]: reads and validates a saved workbook.

use autumn_web::AutumnError;
use autumn_web::reexports::axum::body::to_bytes;
use autumn_web::reexports::axum::extract::{FromRequest, Request};
use autumn_web::reexports::http::{StatusCode, header};

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
/// The responses (Autumn Problem Details):
///
/// - `415`: the content type is not JSON.
/// - `413`: the body is over [`WorkbookSnapshot::MAX_BYTES`].
/// - `400`: the body is not JSON.
/// - `422`: the JSON is not a workbook, or it breaks a [`Workbook`]
///   invariant or the default [`Limits`](crate::Limits).
///
/// The extractor reads at most 8 MiB, because parsing uses about 20 times
/// the body size in memory. For larger sheets or other limits, take
/// `Json<Workbook>` and call [`Workbook::validate_with`].
#[derive(Debug, Clone, PartialEq)]
pub struct WorkbookSnapshot(pub Workbook);

impl WorkbookSnapshot {
    /// The largest body that the extractor reads: 8 MiB.
    pub const MAX_BYTES: usize = 8 * 1024 * 1024;

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

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let is_json = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .map(str::trim)
            .is_some_and(|mime| {
                mime.eq_ignore_ascii_case("application/json")
                    || mime.to_ascii_lowercase().ends_with("+json")
            });
        if !is_json {
            return Err(AutumnError::bad_request_msg(
                "Expected request with `Content-Type: application/json`",
            )
            .with_status(StatusCode::UNSUPPORTED_MEDIA_TYPE));
        }
        let bytes = to_bytes(req.into_body(), Self::MAX_BYTES)
            .await
            .map_err(|err| {
                let message = err.to_string();
                let status = if message.contains("length limit") {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                };
                AutumnError::bad_request_msg(message).with_status(status)
            })?;
        let workbook: Workbook = serde_json::from_slice(&bytes).map_err(|err| {
            let status = if err.is_data() {
                StatusCode::UNPROCESSABLE_ENTITY
            } else {
                StatusCode::BAD_REQUEST
            };
            AutumnError::bad_request_msg(err.to_string()).with_status(status)
        })?;
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
    async fn rejects_a_wrong_shape_with_422() {
        post(&json!({ "sheets": [] })).await.assert_status(422);
        post(&json!({ "id": "w", "sheets": { "s": { "id": "s", "cellData": { "x": {} } } } }))
            .await
            .assert_status(422);
    }

    #[tokio::test]
    async fn accepts_json_content_types_with_parameters() {
        let body = json!({ "id": "wb" }).to_string();
        for ct in [
            "application/json; charset=utf-8",
            "application/merge-patch+json",
        ] {
            client()
                .post("/save")
                .header("content-type", ct)
                .body(body.clone())
                .send()
                .await
                .assert_ok();
        }
        client()
            .post("/save")
            .header("content-type", "text/plain")
            .body(body)
            .send()
            .await
            .assert_status(415);
    }

    #[tokio::test]
    async fn rejects_bodies_over_the_cap_with_413() {
        let big = format!(
            r#"{{"id":"w","pad":"{}"}}"#,
            "x".repeat(WorkbookSnapshot::MAX_BYTES)
        );
        client()
            .post("/save")
            .header("content-type", "application/json")
            .body(big)
            .send()
            .await
            .assert_status(413);
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
