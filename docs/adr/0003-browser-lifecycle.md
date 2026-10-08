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
- Saves are debounced and versioned (an edit during a save keeps the
  sheet dirty), with the Autumn CSRF header.
- JS maps track mounted and mounting sheets (not DOM markers). Each mount
  has a token; a dispose or a removal during a mount frees the new
  instance. A copy of live markup (htmx history) mounts again.
- Saves run in one ordered chain per sheet. Waiting calls share one
  request. Dispose queues the last snapshot after the save in progress.
  Only `pagehide` uses `keepalive`, with one byte budget for all sheets.
- Load and save URLs must be same-origin; saves use `redirect: "error"`.
- Multi-sheet pages: only the active sheet (last `pointerdown` or
  `focusin`) keeps Univer's element ids. The others get the suffix
  `--autumn-inactive`.

## Consequences

- Each workaround has a system test that fails without it.
- The id swap depends on Univer internals. A Univer update must re-run the
  system tests. Remove the swap when Univer scopes its ids.
