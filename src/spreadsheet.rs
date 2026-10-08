//! [`Spreadsheet`]: the mount markup for one Univer workbook.
//!
//! The markup holds only data attributes and a JSON data block. It holds no
//! inline script and no `style=` attribute, so it works with the default
//! Autumn CSP and with nonce mode. `init.js` reads the attributes and
//! mounts Univer.

use std::time::Duration;

use autumn_web::{Markup, PreEscaped, html};
use maud::Render;

use crate::locale::Locale;
use crate::workbook::Workbook;

/// The autosave delay when the app sets a save URL and no delay.
const DEFAULT_AUTOSAVE: Duration = Duration::from_millis(1000);

/// Where the workbook data comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// No data: Univer makes one empty sheet.
    Empty,
    /// Workbook JSON in the page.
    Inline(String),
    /// A URL that returns workbook JSON.
    Url(String),
}

/// The mount markup for one Univer workbook.
///
/// ```rust
/// use autumn_plugin_univer::{Spreadsheet, Workbook};
///
/// let html = Spreadsheet::new("budget")
///     .workbook(&Workbook::empty("budget"))
///     .save_url("/budget")
///     .height("60vh")
///     .render()
///     .into_string();
/// assert!(html.contains(r#"data-univer-save-url="/budget""#));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Spreadsheet {
    id: String,
    label: Option<String>,
    height: Option<String>,
    locale: Locale,
    read_only: bool,
    dark_mode: bool,
    header: bool,
    toolbar: bool,
    footer: bool,
    formula_bar: bool,
    context_menu: bool,
    source: Source,
    save_url: Option<String>,
    autosave: Option<Duration>,
}

impl Spreadsheet {
    /// Starts the markup for the element with this DOM `id`.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: None,
            height: None,
            locale: Locale::default(),
            read_only: false,
            dark_mode: false,
            header: true,
            toolbar: true,
            footer: true,
            formula_bar: true,
            context_menu: true,
            source: Source::Empty,
            save_url: None,
            autosave: None,
        }
    }

    /// Puts this workbook in the page as JSON.
    pub fn workbook(mut self, workbook: &Workbook) -> Self {
        self.source = Source::Inline(workbook.to_json());
        self
    }

    /// Loads the workbook JSON from this URL (`GET`, same origin).
    pub fn load_url(mut self, url: impl Into<String>) -> Self {
        self.source = Source::Url(url.into());
        self
    }

    /// POSTs the snapshot JSON to this URL after edits. Read it with
    /// [`WorkbookSnapshot`](crate::WorkbookSnapshot).
    pub fn save_url(mut self, url: impl Into<String>) -> Self {
        self.save_url = Some(url.into());
        self
    }

    /// Sets the autosave delay after the last edit. The default is 1 s.
    pub const fn autosave(mut self, delay: Duration) -> Self {
        self.autosave = Some(delay);
        self
    }

    /// Turns off autosave. Save with a [`save_button`] or
    /// `AutumnUniver.save(element)`.
    pub const fn manual_save(mut self) -> Self {
        self.autosave = Some(Duration::ZERO);
        self
    }

    /// Sets the height as a CSS length (`480px`, `60vh`, `30rem`, …). The
    /// default is `480px`. An invalid length is ignored.
    pub fn height(mut self, height: impl Into<String>) -> Self {
        let height = height.into();
        self.height = is_css_length(&height).then_some(height);
        self
    }

    /// Sets the accessible name (`aria-label`).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the UI language. The default is `en-US`.
    pub const fn locale(mut self, locale: Locale) -> Self {
        self.locale = locale;
        self
    }

    /// Blocks edits when `true`. A read-only sheet does not save.
    pub const fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Uses the Univer dark theme when `true`.
    pub const fn dark_mode(mut self, dark_mode: bool) -> Self {
        self.dark_mode = dark_mode;
        self
    }

    /// Shows the header (menu and toolbar). The default is `true`.
    pub const fn header(mut self, show: bool) -> Self {
        self.header = show;
        self
    }

    /// Shows the toolbar. The default is `true`.
    pub const fn toolbar(mut self, show: bool) -> Self {
        self.toolbar = show;
        self
    }

    /// Shows the footer (sheet tabs and zoom). The default is `true`.
    pub const fn footer(mut self, show: bool) -> Self {
        self.footer = show;
        self
    }

    /// Shows the formula bar. The default is `true`.
    pub const fn formula_bar(mut self, show: bool) -> Self {
        self.formula_bar = show;
        self
    }

    /// Shows the context menu. The default is `true`.
    pub const fn context_menu(mut self, show: bool) -> Self {
        self.context_menu = show;
        self
    }

    /// Renders the markup.
    pub fn render(&self) -> Markup {
        html! {}
    }
}

impl Render for Spreadsheet {
    fn render(&self) -> Markup {
        Self::render(self)
    }
}

/// Renders a button that saves the spreadsheet with this DOM id.
///
/// It needs no JavaScript of your own: `init.js` handles the click.
///
/// ```rust
/// use autumn_plugin_univer::save_button;
///
/// let html = save_button("budget", "Save").into_string();
/// assert!(html.contains(r#"data-univer-save-for="budget""#));
/// ```
pub fn save_button(spreadsheet_id: &str, label: &str) -> Markup {
    let _ = (spreadsheet_id, label);
    html! {}
}

/// `true` for a plain CSS length: a number and one unit, or a percentage.
fn is_css_length(s: &str) -> bool {
    let _ = s;
    false
}

/// Escapes JSON for a `<script type="application/json">` block.
///
/// The HTML parser ends a script block at `</script`, and older parsers
/// treat `<!--` as special. JSON allows `\u` escapes in strings, and these
/// characters occur only in strings, so the escape keeps the data equal.
fn escape_json(json: &str) -> String {
    json.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbook::{Cell, Sheet};
    use proptest::prelude::*;

    fn html(s: &Spreadsheet) -> String {
        s.render().into_string()
    }

    #[test]
    fn default_markup_has_the_marker_and_the_mount() {
        let out = html(&Spreadsheet::new("s1"));
        assert!(out.starts_with("<div"), "{out}");
        assert!(out.contains(r#"id="s1""#), "{out}");
        assert!(out.contains(r#"class="autumn-univer""#), "{out}");
        assert!(out.contains("data-univer "), "{out}");
        assert!(out.contains(r#"data-univer-locale="en-US""#), "{out}");
        assert!(out.contains("data-univer-mount"), "{out}");
        assert!(out.contains("<noscript>"), "{out}");
        assert!(out.contains(r#"role="region""#), "{out}");
        for absent in [
            "data-univer-save-url",
            "data-univer-load-url",
            "data-univer-readonly",
            "data-univer-dark",
            "data-univer-height",
            "data-univer-data",
            "data-univer-autosave",
            "=\"false\"",
            "aria-label",
        ] {
            assert!(!out.contains(absent), "{absent} in {out}");
        }
    }

    #[test]
    fn markup_has_no_inline_code_or_style() {
        let out = html(
            &Spreadsheet::new("s")
                .workbook(&Workbook::empty("w"))
                .height("50vh")
                .save_url("/s"),
        );
        assert!(!out.contains("style="), "{out}");
        assert!(!out.contains("onclick"), "{out}");
        assert_eq!(out.matches("<script").count(), 1, "{out}");
        assert!(out.contains(r#"<script type="application/json" data-univer-data>"#), "{out}");
    }

    #[test]
    fn options_become_data_attributes() {
        let out = html(
            &Spreadsheet::new("s")
                .label("Budget")
                .height("60vh")
                .read_only(true)
                .dark_mode(true)
                .header(false)
                .toolbar(false)
                .footer(false)
                .formula_bar(false)
                .context_menu(false)
                .load_url("/data.json")
                .save_url("/save")
                .autosave(Duration::from_millis(2500)),
        );
        for want in [
            r#"aria-label="Budget""#,
            r#"data-univer-height="60vh""#,
            "data-univer-readonly",
            "data-univer-dark",
            r#"data-univer-header="false""#,
            r#"data-univer-toolbar="false""#,
            r#"data-univer-footer="false""#,
            r#"data-univer-formula-bar="false""#,
            r#"data-univer-context-menu="false""#,
            r#"data-univer-load-url="/data.json""#,
            r#"data-univer-save-url="/save""#,
            r#"data-univer-autosave="2500""#,
        ] {
            assert!(out.contains(want), "{want} in {out}");
        }
        assert!(!out.contains("data-univer-data"), "load URL replaces inline data: {out}");
    }

    #[test]
    fn save_url_sets_the_default_autosave() {
        let out = html(&Spreadsheet::new("s").save_url("/s"));
        assert!(out.contains(r#"data-univer-autosave="1000""#), "{out}");
        let manual = html(&Spreadsheet::new("s").save_url("/s").manual_save());
        assert!(manual.contains(r#"data-univer-autosave="0""#), "{manual}");
    }

    #[test]
    fn inline_workbook_is_valid_json() {
        let wb = Workbook::builder("w", "W")
            .sheet(Sheet::new("a", "A").with_cell(0, 0, Cell::text("hi")))
            .build()
            .expect("valid");
        let out = html(&Spreadsheet::new("s").workbook(&wb));
        let start = out.find("data-univer-data>").expect("block") + "data-univer-data>".len();
        let end = out[start..].find("</script>").expect("end") + start;
        let back: Workbook = serde_json::from_str(&out[start..end]).expect("JSON");
        assert_eq!(back, wb);
    }

    #[test]
    fn script_breakout_text_is_escaped() {
        let wb = Workbook::builder("w", "W")
            .sheet(Sheet::new("a", "A").with_cell(
                0,
                0,
                Cell::text("</script><script>alert(1)</script><!-- & \u{2028}\u{2029}"),
            ))
            .build()
            .expect("valid");
        let out = html(&Spreadsheet::new("s").workbook(&wb));
        assert_eq!(out.matches("</script>").count(), 1, "{out}");
        assert!(!out.contains("<!--"), "{out}");
        assert!(!out.contains('\u{2028}'), "{out}");
        assert!(out.contains(r"</script>"), "{out}");
    }

    #[test]
    fn attribute_values_are_html_escaped() {
        let out = html(&Spreadsheet::new(r#"a"b"#).save_url(r#"/x?a=1&b="2""#));
        assert!(out.contains(r#"id="a&quot;b""#), "{out}");
        assert!(out.contains("&amp;b=&quot;2&quot;"), "{out}");
    }

    #[test]
    fn empty_id_omits_the_id_attribute() {
        assert!(!html(&Spreadsheet::new("")).contains("id="));
    }

    #[test]
    fn invalid_heights_are_ignored() {
        for bad in ["", "480", "red", "1px; color: red", "-5px", "10 px", "calc(1px)", "1e3px"] {
            let out = html(&Spreadsheet::new("s").height(bad));
            assert!(!out.contains("data-univer-height"), "{bad:?}: {out}");
        }
        for good in ["480px", "60vh", "30rem", "2.5em", "100%", "50vw"] {
            assert!(is_css_length(good), "{good}");
        }
    }

    #[test]
    fn locale_sets_the_code() {
        let out = html(&Spreadsheet::new("s").locale(Locale::EnUs));
        assert!(out.contains(r#"data-univer-locale="en-US""#), "{out}");
    }

    #[test]
    fn spreadsheets_render_inside_html_macros() {
        let sheet = Spreadsheet::new("s");
        let out = html! { main { (sheet) } }.into_string();
        assert!(out.contains("data-univer"), "{out}");
    }

    #[test]
    fn save_buttons_target_their_sheet() {
        let out = save_button("budget", "Save now").into_string();
        assert!(out.contains(r#"type="button""#), "{out}");
        assert!(out.contains(r#"data-univer-save-for="budget""#), "{out}");
        assert!(out.contains("Save now"), "{out}");
        assert!(!out.contains("onclick"), "{out}");
    }

    proptest! {
        #[test]
        fn escaped_json_parses_to_the_same_value(s in any::<String>()) {
            let json = serde_json::to_string(&s).expect("ser");
            let escaped = escape_json(&json);
            prop_assert!(!escaped.contains('<'));
            prop_assert!(!escaped.contains('>'));
            prop_assert!(!escaped.contains('&'));
            prop_assert!(!escaped.contains(['\u{2028}', '\u{2029}']), "line separators");
            let back: String = serde_json::from_str(&escaped).expect("de");
            prop_assert_eq!(back, s);
        }
    }
}
