//! Content tests for `assets/init.js`. The Chromium system tests
//! (`tests/system.rs`) cover its behavior.

const INIT: &str = include_str!("../assets/init.js");

#[test]
fn mounts_on_load_and_on_htmx_swaps() {
    assert!(INIT.contains("[data-univer]"));
    assert!(INIT.contains("htmx:load"));
    assert!(INIT.contains("data-univer-init"), "marks mounted nodes");
}

#[test]
fn disposes_on_htmx_cleanup_and_on_removal() {
    assert!(INIT.contains("htmx:beforeCleanupElement"));
    assert!(INIT.contains("MutationObserver"));
    assert!(INIT.contains(".dispose()"));
}

#[test]
fn saves_with_the_autumn_csrf_header() {
    assert!(INIT.contains(r#"meta[name="csrf-token"]"#));
    assert!(INIT.contains("X-CSRF-Token"));
    assert!(INIT.contains(r#"method: "POST""#));
    assert!(INIT.contains("same-origin"));
}

#[test]
fn emits_the_documented_events() {
    for event in ["ready", "change", "saving", "saved", "save-error", "error"] {
        assert!(INIT.contains(&format!(r#""{event}""#)), "{event}");
    }
    assert!(INIT.contains(r#""autumn-univer:""#));
}

#[test]
fn reads_every_spreadsheet_attribute() {
    for attr in [
        "locale", "height", "readonly", "dark", "header", "toolbar", "footer",
        "formula-bar", "context-menu", "load-url", "save-url", "autosave",
    ] {
        assert!(INIT.contains(&format!(r#""{attr}""#)), "{attr}");
    }
    assert!(INIT.contains("data-univer-data"));
    assert!(INIT.contains("data-univer-save-for"));
}

#[test]
fn uses_no_unsafe_dom_or_eval() {
    for banned in ["eval(", "new Function", "innerHTML", "document.write", "outerHTML"] {
        assert!(!INIT.contains(banned), "{banned}");
    }
}

#[test]
fn read_only_sheets_block_edits() {
    assert!(INIT.contains("setEditable(false)"));
    assert!(INIT.contains("getWorkbookPermission().setReadOnly()"), "blocks typing too");
}

#[test]
fn counts_only_mutations_of_its_own_workbook() {
    assert!(INIT.contains("event.type !== MUTATION"));
    assert!(INIT.contains("event.params.unitId !== unitId"));
}
