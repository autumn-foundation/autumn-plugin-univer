# Acceptance criteria and evidence

The repository has no issue. The acceptance criteria (AC) come from
[`plan.md`](plan.md). Each row names the proof. CI runs every test below
(`.github/workflows/ci.yml`).

| AC | Criterion | Evidence |
|---|---|---|
| AC1 | `UniverPlugin` installs one `PluginAssets` bundle (`univer`) with hashed immutable URLs, SRI, ETag/304, and passes Autumn conformance. | `src/plugin.rs`: `univer_js_serves_at_its_fingerprinted_url`, `plugin_files_serve_at_their_fingerprinted_urls`, `lazy_chunks_serve_at_their_plain_urls`, `plain_urls_answer_conditional_requests`, `unbundled_and_stale_paths_are_not_found`, `bundle_routes_are_public_plugin_routes`, `plugin_passes_conformance`, `asset_url_resolves_the_installed_bundle`. `src/assets.rs`: `integrity_is_computed_from_the_bytes`. |
| AC2 | `univer_stylesheet()` and `univer_script()` render tags with SRI, in order. | `src/script.rs`: `script_tags_are_modules_with_sri`, `univer_loads_before_the_init_script`, `stylesheet_links_carry_sri`, `plugin_css_loads_after_univer_css`, `script_tags_list_the_compiled_locales`. |
| AC3 | A typed `Spreadsheet` builder renders the mount markup with every option. | `src/spreadsheet.rs`: `default_markup_has_the_marker_and_the_mount`, `options_become_data_attributes`, `save_url_sets_the_default_autosave`, `autosave_delays_stay_in_the_timer_range`, `invalid_heights_are_ignored`, `save_buttons_target_their_sheet`, `spreadsheets_render_inside_html_macros`. `src/init_js_tests.rs`: `reads_every_spreadsheet_attribute`. |
| AC4 | Inline JSON is safe against `</script>` injection. | `src/spreadsheet.rs`: `script_breakout_text_is_escaped`, `attribute_values_are_html_escaped`, property test `escaped_json_parses_to_the_same_value`. |
| AC5 | A Rust `Workbook` model seeds data and reads snapshots with no data loss. | `src/workbook.rs`: `reads_a_univer_snapshot` (real Univer 1.0.3 snapshot), `round_trip_keeps_every_field`, property test `built_workbooks_are_valid_and_round_trip`, `rows_fill_from_column_a`, `builder_keeps_tab_order_and_replaces_same_ids`, `cell_values_read_only_scalars`; CI runs the model tests with `serde_json/arbitrary_precision` too. |
| AC6 | Invariants are specified and enforced. | Spec: `plan.md` ("Invariant spec") and the `workbook.rs` module doc (8 invariants). Tests: `rejects_*` (one per invariant), `extra_must_not_shadow_typed_fields`, `enforces_limits`, `cell_count_is_checked_before_cell_bounds`, `non_canonical_cell_indexes_are_errors`, property tests `shrinking_below_a_cell_is_invalid`, `order_must_be_a_permutation`, `arbitrary_json_never_panics`. Verus is not available here (see "Gaps"). |
| AC7 | `WorkbookSnapshot` validates posted snapshots; `422` on bad input. | `src/extract.rs`: `accepts_a_valid_snapshot`, `rejects_broken_invariants_with_422`, `rejects_out_of_bounds_cells_with_422`, `rejects_a_wrong_shape_with_422`, `errors_name_the_json_path`, `rejects_bad_json_with_400`, `rejects_trailing_characters_with_400`, `rejects_a_missing_content_type`, `rejects_non_application_json_suffixes`, `rejects_bodies_over_the_cap_with_413`, `keeps_the_app_body_limit_when_it_is_lower`. |
| AC8 | Real browser, default CSP: the sheet mounts with no console errors. | `tests/system.rs`: `mounts_under_the_default_csp_with_no_console_errors` (formula `=A1*2` computes 84), `two_sheets_mount_on_one_page`, `load_url_fetches_the_workbook`. `src/plugin.rs`: `pages_keep_the_strict_default_csp`. |
| AC9 | An edit triggers a debounced POST with CSRF; events fire. | `tests/system.rs`: `an_edit_autosaves_with_the_csrf_token` (CSRF on), `a_save_without_the_csrf_token_reports_an_error` (403 → `save-error`), `the_save_button_saves_a_manual_sheet`, `edits_during_a_save_and_before_dispose_all_arrive_in_order`, `rapid_manual_saves_run_one_at_a_time`, `dispose_sends_no_duplicate_of_an_in_flight_save`, `unload_flushes_a_save_queued_by_dispose`, `a_cross_origin_save_url_is_refused`. |
| AC10 | Read-only mode blocks edits. | `tests/system.rs`: `read_only_sheets_block_edits_and_never_save`. Real typing was also checked in Chromium on the demo report page (value unchanged). |
| AC11 | htmx-swapped sheets mount; removed sheets dispose. | `tests/system.rs`: `htmx_swaps_mount_and_cleanup_disposes`, `dispose_during_loading_stops_the_mount`, `a_remount_during_a_mount_renders`, `a_copy_of_mounted_markup_mounts_again` (htmx history), `a_sheet_moved_later_mounts_again`, `a_reattached_sheet_keeps_its_unsaved_edits`, `bad_data_fails_cleanly`, `only_the_active_sheet_keeps_the_editor_ids`. |
| AC12 | All 19 Univer locales load on demand. | Cargo features `locale-*` / `all-locales`; `src/locale.rs`: `every_compiled_locale_has_its_chunk`, `all_locales_builds_hold_nineteen_locales`; `tests/system.rs`: `a_bundled_locale_loads_lazily` (fr-FR chunk fetched on demand), `a_locale_missing_from_the_build_is_not_fetched`, `unknown_locales_fall_back_to_en_us`. |
| AC13 | Reproducible vendored bundle, provenance and license notices. | `vendor/` (exact pins, lockfile, `build.mjs`); CI job `vendor` rebuilds and fails on a diff; `src/assets.rs`: `vendor_bytes_match_the_manifest_hashes`, `dist_dir_has_no_files_outside_the_manifest`, `every_static_import_is_compiled_in`, `files_of_disabled_features_are_not_compiled_in`, `license_notices_cover_every_bundled_package`. `.gitattributes` keeps bytes stable. |
| AC14 | A runnable demo shows seed, edit, save and reload. | `examples/univer_demo.rs`. Driven in Chromium with real mouse and keyboard: edit B2 → autosave → reload keeps 1500 → scratch sheet (htmx, manual save) → back to the budget → report table and read-only view; no console errors. |
| AC15 | Short ASD-STE100 docs: README, ADRs, doc comments. | `README.md`, `docs/plan.md`, `docs/adr/0001…0003`, `CLAUDE.md`; `missing_docs = "warn"` and `cargo doc -D warnings` in CI. |
| AC16 | CI: fmt, clippy (pedantic, nursery), tests, system tests, coverage. | `.github/workflows/ci.yml` jobs `fmt`, `clippy`, `test`, `system`, `coverage` (`--fail-under-lines 90`), `msrv`, `vendor`, `package`. All green on the PR. |

## Gaps

- **Verus proof (user standard).** Verus could not be installed: GitHub
  release downloads are blocked in this environment. The invariant spec
  is written down, enforced at run time and checked by property tests.
  The proof is a follow-up.
- **Univer upstream behavior.** Three workarounds depend on Univer 1.0
  internals (read-only permission timing, editor DOM ids, dispose before
  Rendered). A Univer update must re-run the system tests.
