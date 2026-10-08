//! The `<head>` tags: stylesheets and module scripts.
//!
//! Every tag points at the content-hashed URL from [`UNIVER_ASSETS`] and
//! carries the SRI hash that Autumn computes from the embedded bytes.

use autumn_web::{Markup, html};

use crate::assets::{INIT_JS, PLUGIN_CSS, UNIVER_ASSETS, UNIVER_CSS, UNIVER_JS};

/// Renders one `<script type="module">` tag for a bundled file.
fn module_tag(path: &str) -> Markup {
    match UNIVER_ASSETS.get(path) {
        Some(asset) => html! {
            script type="module" src=(asset.url()) integrity=(asset.integrity()) crossorigin="anonymous" {}
        },
        // Unreachable: the paths are constants of the bundle. A test checks it.
        None => UNIVER_ASSETS.script_tag(path),
    }
}

/// Renders the `<script>` tags that load Univer and the mount script.
///
/// Module scripts run after parsing, in document order. Univer runs first
/// and sets `globalThis.AutumnUniverLib`. Then `init.js` mounts every
/// [`Spreadsheet`](crate::Spreadsheet) on the page.
///
/// ```rust
/// use autumn_plugin_univer::univer_script;
///
/// let html = univer_script().into_string();
/// assert!(html.contains(r#"type="module""#));
/// assert!(html.contains("/static/_plugins/univer/univer."));
/// ```
#[must_use]
pub fn univer_script() -> Markup {
    html! {
        (module_tag(UNIVER_JS))
        (module_tag(INIT_JS))
    }
}

/// Renders the `<link>` tags for the Univer and plugin stylesheets.
///
/// ```rust
/// use autumn_plugin_univer::univer_stylesheet;
///
/// let html = univer_stylesheet().into_string();
/// assert!(html.contains(r#"rel="stylesheet""#));
/// ```
#[must_use]
pub fn univer_stylesheet() -> Markup {
    html! {
        (UNIVER_ASSETS.stylesheet_tag(UNIVER_CSS))
        (UNIVER_ASSETS.stylesheet_tag(PLUGIN_CSS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(path: &str) -> &'static autumn_web::assets::PluginAsset {
        UNIVER_ASSETS.get(path).expect("bundled")
    }

    #[test]
    fn script_tags_are_modules_with_sri() {
        let html = univer_script().into_string();
        for path in [UNIVER_JS, INIT_JS] {
            let a = asset(path);
            assert!(html.contains(&format!(r#"src="{}""#, a.url())), "{html}");
            assert!(html.contains(&format!(r#"integrity="{}""#, a.integrity())), "{html}");
        }
        assert_eq!(html.matches(r#"type="module""#).count(), 2, "{html}");
        assert_eq!(html.matches(r#"crossorigin="anonymous""#).count(), 2, "{html}");
        assert!(!html.contains("not found"), "{html}");
    }

    #[test]
    fn univer_loads_before_the_init_script() {
        let html = univer_script().into_string();
        let univer = html.find(asset(UNIVER_JS).url()).expect("univer tag");
        let init = html.find(asset(INIT_JS).url()).expect("init tag");
        assert!(univer < init, "init.js needs AutumnUniverLib: {html}");
    }

    #[test]
    fn stylesheet_links_carry_sri() {
        let html = univer_stylesheet().into_string();
        for path in [UNIVER_CSS, PLUGIN_CSS] {
            let a = asset(path);
            assert!(html.contains(&format!(r#"href="{}""#, a.url())), "{html}");
            assert!(html.contains(&format!(r#"integrity="{}""#, a.integrity())), "{html}");
        }
        assert_eq!(html.matches(r#"rel="stylesheet""#).count(), 2, "{html}");
    }

    #[test]
    fn plugin_css_loads_after_univer_css() {
        let html = univer_stylesheet().into_string();
        let univer = html.find(asset(UNIVER_CSS).url()).expect("univer css");
        let plugin = html.find(asset(PLUGIN_CSS).url()).expect("plugin css");
        assert!(univer < plugin, "plugin rules win ties: {html}");
    }

    #[test]
    fn missing_paths_fall_back_to_a_comment() {
        let html = module_tag("nope.js").into_string();
        assert!(html.contains("not found"), "{html}");
    }
}
