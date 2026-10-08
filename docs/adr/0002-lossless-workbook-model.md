# ADR 0002: A lossless workbook model with checked invariants

- Status: accepted
- Date: 2026-10-08

## Context

Apps seed sheets from Rust data and store what the browser saves. Univer's
`IWorkbookData` has many fields (styles, merges, row data, resources). A
fully typed model drops fields that it does not know, and a new
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
- `Workbook::validate_with(&Limits)` checks eight invariants (see
  `docs/plan.md`). Invariant 8 stops an `extra` key from shadowing a typed
  field. `WorkbookBuilder::build` validates. `WorkbookSnapshot` reads at
  most 8 MiB, then validates and returns `422`.
- `CellValue` reads numbers through `serde_json::Value`, so the model also
  works when a host turns on `serde_json/arbitrary_precision`.
- `cellData` keys must be canonical decimals (`"01"` is an error), so two
  keys never name one cell.
- `Sheet::to_rows` has a grid cap, because one far cell makes a huge grid.

## Consequences

- `float_roundtrip` reaches the host through feature unification. It makes
  float parsing exact and a little slower.
- A Univer snapshot round-trips with no data loss (tested with a real 1.0.3
  snapshot and property tests).
- Verus was not available in the build environment. The invariant spec is
  in `docs/plan.md`; property tests check
  `build() = Ok(w) ⇒ valid(w)` and that broken orders and bounds are
  rejected.
