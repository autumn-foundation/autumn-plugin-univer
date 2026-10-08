//! [`UniverPlugin`]: installs the Univer assets in an Autumn app.

use std::borrow::Cow;

use autumn_web::app::AppBuilder;
use autumn_web::plugin::Plugin;

use crate::assets::UNIVER_ASSETS;

/// The plugin name in Autumn diagnostics.
pub const PLUGIN_NAME: &str = "autumn-plugin-univer";

/// Installs the Univer assets in an Autumn app.
///
/// ```rust,no_run
/// use autumn_plugin_univer::UniverPlugin;
///
/// # async fn run() {
/// autumn_web::app()
///     .plugin(UniverPlugin::new())
///     .run()
///     .await;
/// # }
/// ```
///
/// Then put [`univer_stylesheet`](crate::univer_stylesheet) and
/// [`univer_script`](crate::univer_script) in the page, and render a
/// [`Spreadsheet`](crate::Spreadsheet).
#[derive(Debug, Default)]
#[must_use]
pub struct UniverPlugin;

impl UniverPlugin {
    /// Makes the plugin. It reads no configuration.
    pub const fn new() -> Self {
        Self
    }
}

impl Plugin for UniverPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(PLUGIN_NAME)
    }

    fn build(self, app: AppBuilder) -> AppBuilder {
        app.plugin_assets(&UNIVER_ASSETS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{INIT_JS, LICENSES, PLUGIN_CSS, UNIVER_CSS, UNIVER_JS};
    use autumn_web::assets::{PLUGIN_ASSETS_ROUTE_MARKER, asset_url};
    use autumn_web::plugin_conformance::{ConformanceConfig, run_conformance};
    use autumn_web::route_listing::{RouteClassification, RouteSource};
    use autumn_web::test::{TestApp, TestClient};

    const JS: &str = "text/javascript; charset=utf-8";
    const CSS: &str = "text/css; charset=utf-8";
    const IMMUTABLE: &str = "public, max-age=31536000, immutable";
    const REVALIDATE: &str = "public, max-age=0, must-revalidate";

    fn client() -> TestClient {
        TestApp::new().plugin(UniverPlugin::new()).build()
    }

    fn url(path: &str) -> String {
        UNIVER_ASSETS.url(path)
    }

    #[tokio::test]
    async fn univer_js_serves_at_its_fingerprinted_url() {
        let response = client().get(&url(UNIVER_JS)).send().await;
        response
            .assert_ok()
            .assert_header("content-type", JS)
            .assert_header("cache-control", IMMUTABLE);
        assert!(response.body.starts_with(b"import"), "ES module entry");
    }

    #[tokio::test]
    async fn plugin_files_serve_at_their_fingerprinted_urls() {
        let client = client();
        let init = client.get(&url(INIT_JS)).send().await;
        init.assert_ok().assert_header("content-type", JS);
        assert!(init.text().contains("data-univer"), "init.js scans the marker");
        client
            .get(&url(PLUGIN_CSS))
            .send()
            .await
            .assert_ok()
            .assert_header("content-type", CSS);
        client
            .get(&url(UNIVER_CSS))
            .send()
            .await
            .assert_ok()
            .assert_header("content-type", CSS);
        client.get(&url(LICENSES)).send().await.assert_ok();
    }

    #[tokio::test]
    async fn lazy_chunks_serve_at_their_plain_urls() {
        // `univer.js` imports chunks with relative URLs, so the plain URL
        // must work for every chunk.
        let client = client();
        let chunks: Vec<String> = UNIVER_ASSETS
            .iter()
            .map(|a| a.logical_path().to_owned())
            .filter(|p| p.starts_with("chunks/"))
            .collect();
        assert!(!chunks.is_empty());
        for chunk in chunks {
            client
                .get(&format!("/static/_plugins/univer/{chunk}"))
                .send()
                .await
                .assert_ok()
                .assert_header("content-type", JS)
                .assert_header("cache-control", REVALIDATE);
        }
    }

    #[tokio::test]
    async fn plain_urls_answer_conditional_requests() {
        let client = client();
        for path in [UNIVER_JS, INIT_JS, UNIVER_CSS, PLUGIN_CSS] {
            let plain = format!("/static/_plugins/univer/{path}");
            let response = client.get(&plain).send().await;
            response.assert_ok().assert_header("cache-control", REVALIDATE);
            let etag = response.header("etag").expect("etag").to_owned();
            client
                .get(&plain)
                .header("if-none-match", &etag)
                .send()
                .await
                .assert_status(304);
        }
    }

    #[tokio::test]
    async fn unbundled_and_stale_paths_are_not_found() {
        let client = client();
        for path in [
            "/static/_plugins/univer/manifest.json",
            "/static/_plugins/univer/init.00000000.js",
            "/static/_plugins/univer/nope.js",
            "/static/_plugins/univer/../manifest.json",
        ] {
            client.get(path).send().await.assert_status(404);
        }
    }

    #[cfg(not(feature = "locale-fr-fr"))]
    #[tokio::test]
    async fn gated_locales_are_absent_without_their_feature() {
        assert!(
            !UNIVER_ASSETS
                .iter()
                .any(|a| a.logical_path().starts_with("chunks/fr-FR-")),
            "fr-FR needs the locale-fr-fr feature"
        );
    }

    #[tokio::test]
    async fn asset_url_resolves_the_installed_bundle() {
        let _client = client();
        for path in [UNIVER_JS, INIT_JS, UNIVER_CSS, PLUGIN_CSS] {
            assert_eq!(asset_url(&format!("_plugins/univer/{path}")), url(path));
        }
    }

    #[test]
    fn bundle_routes_are_public_plugin_routes() {
        let app = autumn_web::app().plugin(UniverPlugin::new());
        let infos = app.plugin_route_infos().expect("route infos");
        let routes: Vec<_> = infos
            .iter()
            .filter(|i| i.path.starts_with("/static/_plugins/univer/"))
            .collect();
        assert_eq!(routes.len(), UNIVER_ASSETS.iter().count() * 2, "two URLs per file");
        for info in routes {
            assert_eq!(info.method, "GET");
            assert_eq!(info.classification, RouteClassification::Public);
            assert_eq!(info.middleware, [PLUGIN_ASSETS_ROUTE_MARKER]);
            assert_eq!(info.source, RouteSource::Plugin(PLUGIN_NAME.to_owned()));
        }
    }

    #[test]
    fn plugin_passes_conformance() {
        let app = autumn_web::app().plugin(UniverPlugin::new());
        let infos = app.plugin_route_infos().expect("route infos");
        let report = run_conformance(&ConformanceConfig::new(PLUGIN_NAME), &infos);
        assert!(report.passed(), "{}", report.to_text_report());
    }

    #[tokio::test]
    async fn installing_the_plugin_twice_is_harmless() {
        let client = TestApp::new()
            .plugin(UniverPlugin::new())
            .plugin(UniverPlugin::new())
            .build();
        client.get(&url(INIT_JS)).send().await.assert_ok();
    }

    #[test]
    fn plugin_name_is_stable() {
        assert_eq!(UniverPlugin::new().name(), PLUGIN_NAME);
    }
}
