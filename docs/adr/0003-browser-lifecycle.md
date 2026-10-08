# ADR 0003: Browser lifecycle, saving and multi-sheet pages

- Status: accepted
- Date: 2026-10-08

## Context

The page is server-rendered and may change through htmx. The CSP blocks
inline code. System tests found three Univer 1.0 behaviors:

1. `FWorkbook.setEditable(false)` does not block typing.
2. The cell editor and the formula engine send mutations that change no
   workbook data.
3. Univer gives its cell editor a fixed DOM id and finds it with
   `getElementById`. With two sheets, keys go to the first sheet.

## Decision

- The markup holds only data attributes and a JSON data block.
  `init.js` (module script) mounts each `[data-univer]` element on load and
  on `htmx:load`.
- It disposes on `htmx:beforeCleanupElement`, and a `MutationObserver`
  disposes elements that leave the DOM. Unsaved edits go out first
  (`fetch` with `keepalive` when the body is small).
- `ready` fires two frames after the `Rendered` lifecycle stage. Then
  pointer input works.
- Read-only: `setEditable(false)` and
  `getWorkbookPermission().setReadOnly()` after `Rendered`.
- A change is a `MUTATION` whose `params.unitId` is the sheet's workbook.
- Saves are debounced, one at a time, versioned (an edit during a save
  keeps the sheet dirty), with the Autumn CSRF header.
- Multi-sheet pages: only the active sheet (last `pointerdown` or
  `focusin`) keeps Univer's element ids. The others get the suffix
  `--autumn-inactive`.

## Consequences

- Each workaround has a system test that fails without it.
- The id swap depends on Univer internals. A Univer update must re-run the
  system tests. Remove the swap when Univer scopes its ids.
