//! The plugin asset bundle: vendored Univer plus the plugin-authored files.
//!
//! `vendor/build.mjs` builds Univer into `assets/dist/` and writes the file
//! list (`src/bundle_files.rs`) and the provenance record
//! (`assets/manifest.json`). [`UniverPlugin`](crate::UniverPlugin) installs
//! the bundle through the Autumn `AppBuilder::plugin_assets` seam. Autumn
//! computes the URLs and SRI hashes from the embedded bytes.
//!
//! `manifest.json` is not in the bundle, so Autumn does not serve it.

use autumn_web::assets::PluginAssets;

/// URL namespace of the bundle. Files serve under `/static/_plugins/univer/`.
pub const ASSETS_NAMESPACE: &str = "univer";

/// The vendored Univer ES module (entry chunk).
pub(crate) const UNIVER_JS: &str = "univer.js";

/// The vendored Univer stylesheet.
pub(crate) const UNIVER_CSS: &str = "univer.css";

/// The plugin-authored mount script.
pub(crate) const INIT_JS: &str = "init.js";

/// The plugin-authored stylesheet (container layout).
pub(crate) const PLUGIN_CSS: &str = "autumn-univer.css";

/// License notices of the vendored packages.
#[cfg(test)]
pub(crate) const LICENSES: &str = "THIRD-PARTY-LICENSES.txt";

/// The plugin asset bundle.
///
/// [`UniverPlugin`](crate::UniverPlugin) installs it. Use it directly only
/// to make URLs or tags yourself:
///
/// ```rust
/// use autumn_plugin_univer::UNIVER_ASSETS;
///
/// let url = UNIVER_ASSETS.url("init.js");
/// assert!(url.starts_with("/static/_plugins/univer/init."), "{url}");
/// let sri = UNIVER_ASSETS.integrity("univer.js").expect("univer.js is bundled");
/// assert!(sri.starts_with("sha384-"));
/// ```
pub static UNIVER_ASSETS: PluginAssets =
    PluginAssets::from_files(ASSETS_NAMESPACE, crate::bundle_files::FILES);

/// The Univer version in `assets/dist/`.
pub const UNIVER_VERSION: &str = "1.0.3";

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use sha2::{Digest as _, Sha384};
    use std::collections::BTreeSet;

    const MANIFEST: &str = include_str!("../assets/manifest.json");

    fn manifest() -> serde_json::Value {
        serde_json::from_str(MANIFEST).expect("manifest.json is JSON")
    }

    fn sri(bytes: &[u8]) -> String {
        format!(
            "sha384-{}",
            base64::engine::general_purpose::STANDARD.encode(Sha384::digest(bytes))
        )
    }

    fn compiled() -> BTreeSet<String> {
        UNIVER_ASSETS
            .iter()
            .map(|a| a.logical_path().to_owned())
            .collect()
    }

    /// Manifest files with no feature gate.
    fn ungated() -> BTreeSet<String> {
        let m = manifest();
        let gated: BTreeSet<String> = m["features"]
            .as_object()
            .expect("features map")
            .values()
            .flat_map(|files| files.as_array().expect("file list").iter())
            .map(|f| f.as_str().expect("path").to_owned())
            .collect();
        m["files"]
            .as_object()
            .expect("files map")
            .keys()
            .filter(|f| !gated.contains(*f))
            .cloned()
            .collect()
    }

    #[test]
    fn bundle_holds_the_plugin_files_and_the_core_vendor_files() {
        let files = compiled();
        for path in [UNIVER_JS, UNIVER_CSS, INIT_JS, PLUGIN_CSS, LICENSES] {
            assert!(files.contains(path), "{path} is bundled");
        }
        assert!(!files.contains("manifest.json"), "provenance is not served");
        assert_eq!(UNIVER_ASSETS.namespace(), ASSETS_NAMESPACE);
        assert_eq!(UNIVER_ASSETS.mount_path(), "/static/_plugins/univer");
    }

    #[test]
    fn every_ungated_vendor_file_is_compiled_in() {
        let files = compiled();
        for path in ungated() {
            assert!(files.contains(&path), "{path} is always bundled");
        }
    }

    #[test]
    fn the_en_us_locale_is_always_compiled_in() {
        assert!(
            compiled().iter().any(|p| p.starts_with("chunks/en-US-")),
            "en-US is the fallback locale"
        );
    }

    #[test]
    fn vendor_bytes_match_the_manifest_hashes() {
        let m = manifest();
        let pinned = m["files"].as_object().expect("files map");
        for asset in UNIVER_ASSETS.iter() {
            let path = asset.logical_path();
            if path == INIT_JS || path == PLUGIN_CSS {
                assert!(!pinned.contains_key(path), "plugin files are not pinned");
                continue;
            }
            assert_eq!(
                pinned.get(path).and_then(|v| v.as_str()),
                Some(sri(asset.bytes()).as_str()),
                "{path} matches its manifest hash"
            );
        }
    }

    #[test]
    fn dist_dir_has_no_files_outside_the_manifest() {
        let m = manifest();
        let pinned: BTreeSet<String> = m["files"]
            .as_object()
            .expect("files map")
            .keys()
            .cloned()
            .collect();
        let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dist");
        let mut on_disk = BTreeSet::new();
        let mut stack = vec![dist.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read dist") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let rel = path.strip_prefix(&dist).expect("under dist");
                    on_disk.insert(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        assert_eq!(on_disk, pinned, "re-run vendor/build.mjs");
    }

    #[test]
    fn manifest_agrees_with_the_version_constant() {
        let m = manifest();
        assert_eq!(m["version"], UNIVER_VERSION);
        assert_eq!(m["license"], "Apache-2.0");
        assert_eq!(m["packages"]["@univerjs/presets"], UNIVER_VERSION);
        assert_eq!(
            m["packages"]["@univerjs/preset-sheets-core"],
            UNIVER_VERSION
        );
    }

    #[test]
    fn integrity_is_computed_from_the_bytes() {
        for asset in UNIVER_ASSETS.iter() {
            assert_eq!(
                asset.integrity(),
                sri(asset.bytes()),
                "{}",
                asset.logical_path()
            );
        }
    }

    #[test]
    fn content_types_match_the_files() {
        let ct = |p| UNIVER_ASSETS.get(p).expect("bundled").content_type();
        assert_eq!(ct(UNIVER_JS), "text/javascript; charset=utf-8");
        assert_eq!(ct(INIT_JS), "text/javascript; charset=utf-8");
        assert_eq!(ct(UNIVER_CSS), "text/css; charset=utf-8");
        assert_eq!(ct(PLUGIN_CSS), "text/css; charset=utf-8");
        assert!(ct(LICENSES).starts_with("text/plain"), "{}", ct(LICENSES));
    }

    #[test]
    fn license_notices_cover_univer_and_react() {
        let text = std::str::from_utf8(UNIVER_ASSETS.get(LICENSES).expect("bundled").bytes())
            .expect("utf-8");
        for name in [
            "@univerjs/core@1.0.3",
            "react@18.3.1",
            "react-dom@18.3.1",
            "rxjs@7.8.2",
        ] {
            assert!(text.contains(name), "notice for {name}");
        }
    }

    /// `true` when the Cargo feature is on in this build.
    fn enabled(feature: &str) -> bool {
        match feature {
            "hyphenation" => cfg!(feature = "hyphenation"),
            "locale-ar-sa" => cfg!(feature = "locale-ar-sa"),
            "locale-ca-es" => cfg!(feature = "locale-ca-es"),
            "locale-de-de" => cfg!(feature = "locale-de-de"),
            "locale-es-es" => cfg!(feature = "locale-es-es"),
            "locale-fa-ir" => cfg!(feature = "locale-fa-ir"),
            "locale-fr-fr" => cfg!(feature = "locale-fr-fr"),
            "locale-id-id" => cfg!(feature = "locale-id-id"),
            "locale-it-it" => cfg!(feature = "locale-it-it"),
            "locale-ja-jp" => cfg!(feature = "locale-ja-jp"),
            "locale-ko-kr" => cfg!(feature = "locale-ko-kr"),
            "locale-pl-pl" => cfg!(feature = "locale-pl-pl"),
            "locale-pt-br" => cfg!(feature = "locale-pt-br"),
            "locale-ru-ru" => cfg!(feature = "locale-ru-ru"),
            "locale-sk-sk" => cfg!(feature = "locale-sk-sk"),
            "locale-vi-vn" => cfg!(feature = "locale-vi-vn"),
            "locale-zh-cn" => cfg!(feature = "locale-zh-cn"),
            "locale-zh-hk" => cfg!(feature = "locale-zh-hk"),
            "locale-zh-tw" => cfg!(feature = "locale-zh-tw"),
            other => panic!("unknown feature `{other}` in manifest.json: add it here"),
        }
    }

    #[test]
    fn files_of_disabled_features_are_not_compiled_in() {
        let m = manifest();
        let files = compiled();
        for (feature, gated) in m["features"].as_object().expect("features map") {
            for path in gated.as_array().expect("file list") {
                let path = path.as_str().expect("path");
                // A file shared by two features is in when either is on.
                let wanted =
                    m["features"]
                        .as_object()
                        .expect("features map")
                        .iter()
                        .any(|(f, list)| {
                            enabled(f) && list.as_array().expect("list").iter().any(|p| p == path)
                        });
                assert_eq!(files.contains(path), wanted, "{path} (feature {feature})");
            }
        }
    }

    #[test]
    fn every_static_import_is_compiled_in() {
        // A static import that 404s breaks the module graph in the browser.
        let files = compiled();
        for asset in UNIVER_ASSETS
            .iter()
            .filter(|a| a.content_type().starts_with("text/javascript"))
        {
            let text = std::str::from_utf8(asset.bytes()).expect("utf-8");
            let dir = asset.logical_path().rsplit_once('/').map_or("", |(d, _)| d);
            for marker in ["from\"./", "import\"./"] {
                for (i, _) in text.match_indices(marker) {
                    let rest = &text[i + marker.len()..];
                    let target = rest.split('"').next().expect("closing quote");
                    let path = if dir.is_empty() {
                        target.to_owned()
                    } else {
                        format!("{dir}/{target}")
                    };
                    assert!(
                        files.contains(&path),
                        "{} imports missing {path}",
                        asset.logical_path()
                    );
                }
            }
        }
    }

    #[test]
    fn license_notices_cover_every_bundled_package() {
        let text = std::str::from_utf8(UNIVER_ASSETS.get(LICENSES).expect("bundled").bytes())
            .expect("utf-8");
        let m = manifest();
        for (name, version) in m["packages"].as_object().expect("packages") {
            let heading = format!("{name}@{}", version.as_str().expect("version"));
            assert!(text.contains(&heading), "notice for {heading}");
        }
        assert!(
            !text.contains("ships no license file"),
            "every notice has a full text"
        );
    }

    #[test]
    fn univer_js_exposes_the_library_global() {
        let js = UNIVER_ASSETS.get(UNIVER_JS).expect("bundled").bytes();
        let needle = b"AutumnUniverLib";
        assert!(js.windows(needle.len()).any(|w| w == needle));
    }
}
