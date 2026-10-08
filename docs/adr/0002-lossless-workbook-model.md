# ADR 0002: A lossless workbook model with checked invariants

- Status: accepted
- Date: 2026-10-08

## Context

Apps seed sheets from Rust data and store what the browser saves. Univer's
`IWorkbookData` has many fields (styles, merges, row data, resources). A
fully typed model would drop fields that it does not know, and a new
Univer version adds fields. A save request is untrusted input.

## Decision

- Type the fields that apps use: ids, names, `sheetOrder`, sizes, cells
  (`v`, `t`, `f`, `s`). Keep all other fields in `extra`
  (`#[serde(flatten)]`).
- Newtypes for ids: `WorkbookId`, `SheetId`.
- `cellData` uses a custom (de)serializer. Serde's `flatten` blocks
  integer map keys.
- Integral numbers serialize as JSON integers, as JavaScript does.
  `serde_json/float_roundtrip` keeps floats exact.
- `Workbook::validate_with(&Limits)` checks seven invariants (see
  `docs/plan.md`). `WorkbookBuilder::build` validates. `WorkbookSnapshot`
  validates and returns `422`.

## Consequences

- A Univer snapshot round-trips with no loss (tested with a real 1.0.3
  snapshot and property tests).
- Verus was not available in the build environment. The invariant spec is
  in `docs/plan.md`; property tests check
  `build() = Ok(w) ⇒ valid(w)` and that broken orders and bounds are
  rejected.
