# ADR 0004: One crate, one Cargo feature per editor type

- Status: accepted
- Date: 2026-10-08

## Context

Univer has three editor types: sheets, docs and slides. We can ship them
as one crate or as one crate per type.

Facts (Univer 1.0.3):

- The sheets bundle already holds `core`, `engine-render`, `ui`, `design`,
  `docs` and `docs-ui`. The cell editor uses the docs engine.
- All `@univerjs/*` packages must have the same version.
- Univer gives its editors fixed DOM ids (ADR 0003). Two Univer copies on
  one page also give two DI containers and two sets of global CSS.
- Most of `init.js` is not sheet-specific: mount, dispose, htmx, the save
  chain, CSRF, unload flushes and the re-mount races.

## Decision

- One crate: `autumn-plugin-univer`.
- One Cargo feature per editor type: `sheets` (default), `docs`, `slides`.
  Optional Univer presets get their own features (for example
  `sheets-filter`), in the same way as `locale-*`.
- One esbuild run with one entry point per editor type. The entry points
  share chunks. A page loads only the chunks of the editor types it uses.
- `init.js` has one shared lifecycle. A small adapter per editor type
  creates the Univer instance, reads the snapshot and finds changes.
- The markup tells the type with `data-univer-kind`.
- Proprietary `@univerjs-pro/*` packages are not in this crate.

## Consequences

- An app with sheets and docs loads the shared Univer code one time.
- One version pin keeps all editor types on the same Univer release.
- Lifecycle fixes apply to all editor types.
- A breaking change in one editor type is a breaking change of the crate.
  Before 1.0 this is acceptable.
- Disabled features add no files and no compile time (the same as
  `locale-*` today).
- A different editor engine (not Univer) is a different plugin.
