// autumn-plugin-univer: mounts Univer on each [data-univer] element.
//
// Load order: univer.js (sets globalThis.AutumnUniverLib), then this file.
// Both are module scripts, so they run after parsing, in document order.
//
// Lifecycle: mount on page load, on "htmx:load" and when a sheet enters
// the DOM. Dispose on "htmx:beforeCleanupElement" and when a sheet leaves
// the DOM. Events (on the element, they bubble): autumn-univer:ready,
// change, saving, saved, save-error, error.
//
// JS maps, not DOM attributes, track the sheets. So a copy of the markup
// (for example from the htmx history cache) mounts again.

const LIB = globalThis.AutumnUniverLib;
const PREFIX = "autumn-univer:";
const SELECTOR = "[data-univer]";
const MUTATION = 2; // Univer CommandType.MUTATION
const RENDERED = 2; // Univer LifecycleStages.Rendered
const CSS_LENGTH = /^\d+(\.\d+)?(px|rem|em|vh|vw|%)$/;
const KEEPALIVE_MAX = 60000; // Browsers share 64 KiB for all keepalive bodies.

const states = new Map(); // element -> state of a mounted sheet
const mounting = new Map(); // element -> token of a mount in progress

function attr(el, name) {
  return el.getAttribute("data-univer-" + name);
}

// undefined when absent (Univer default), false when "false".
function shown(el, name) {
  const v = attr(el, name);
  return v === null ? undefined : v !== "false";
}

function has(el, name) {
  return el.hasAttribute("data-univer-" + name);
}

function emit(el, name, detail) {
  el.dispatchEvent(new CustomEvent(PREFIX + name, { bubbles: true, detail }));
}

function setState(el, value) {
  el.setAttribute("data-univer-state", value);
}

function clearMarkers(el) {
  el.removeAttribute("data-univer-init");
  el.removeAttribute("data-univer-state");
}

function mountNode(el) {
  return el.querySelector(":scope > [data-univer-mount]") || el;
}

// The URL, if it has the page's origin. Saves carry the CSRF token, so
// they must not go to another origin.
function sameOrigin(url) {
  if (!url) return null;
  try {
    const resolved = new URL(url, location.href);
    if (resolved.origin === location.origin) return resolved.href;
  } catch {
    // An invalid URL is refused below.
  }
  console.error(`autumn-univer: ${url} is not a same-origin URL; it is ignored`);
  return null;
}

function csrfHeaders() {
  const meta = document.querySelector('meta[name="csrf-token"], meta[name="autumn-csrf-token"]');
  if (!meta) return {};
  return { [meta.getAttribute("data-header") || "X-CSRF-Token"]: meta.getAttribute("content") || "" };
}

// Resolves when the Univer instance reaches `stage` (LifecycleStages).
function whenStage(univerAPI, stage) {
  if (univerAPI.getCurrentLifecycleStage() >= stage) return Promise.resolve();
  return new Promise((resolve) => {
    const subscription = univerAPI.addEvent(univerAPI.Event.LifeCycleChanged, (event) => {
      if (event.stage < stage) return;
      subscription.dispose();
      resolve();
    });
  });
}

function nextFrames(count) {
  return new Promise((resolve) => {
    const step = (left) => (left <= 0 ? resolve() : requestAnimationFrame(() => step(left - 1)));
    step(count);
  });
}

// Locale files in this build (from univer_script()). null: no list.
const BUNDLED = (() => {
  const meta = document.querySelector('meta[name="autumn-univer-locales"]');
  return meta ? new Set((meta.getAttribute("content") || "").split(",")) : null;
})();

async function loadLocale(code) {
  const loader = BUNDLED && !BUNDLED.has(code) ? undefined : LIB.locales[code];
  if (!loader) {
    if (code === "en-US") throw new Error("en-US is missing");
    console.warn(`autumn-univer: locale ${code} is not bundled; using en-US`);
    return loadLocale("en-US");
  }
  try {
    const mod = await loader();
    return { code, pack: mod.default ?? mod };
  } catch (error) {
    if (code === "en-US") throw error;
    console.warn(`autumn-univer: locale ${code} did not load; using en-US`, error);
    return loadLocale("en-US");
  }
}

async function loadData(el) {
  const block = el.querySelector(":scope > script[data-univer-data]");
  if (block) return JSON.parse(block.textContent || "{}");
  const raw = attr(el, "load-url");
  if (!raw) return {};
  const url = sameOrigin(raw);
  if (!url) throw new Error(`load URL ${raw} is not same-origin`);
  const response = await fetch(url, { credentials: "same-origin", headers: { Accept: "application/json" } });
  if (!response.ok) throw new Error(`load ${raw}: HTTP ${response.status}`);
  return response.json();
}

async function mount(el) {
  if (!LIB) {
    console.error("autumn-univer: univer.js did not load before init.js");
    return;
  }
  if (states.has(el) || mounting.has(el)) return;
  const token = {};
  mounting.set(el, token);
  const container = mountNode(el);
  // Markup copied from a live sheet (htmx history) holds dead Univer DOM.
  if (container !== el) container.replaceChildren();
  el.setAttribute("data-univer-init", "");
  setState(el, "loading");
  const height = attr(el, "height");
  if (height && CSS_LENGTH.test(height)) el.style.height = height;
  const live = () => mounting.get(el) === token && el.isConnected;
  let univer = null;
  try {
    const [data, locale] = await Promise.all([loadData(el), loadLocale(attr(el, "locale") || "en-US")]);
    if (!live()) return abort(el, token, univer);
    const type = locale.code.replace("-", ""); // LocaleType: "en-US" -> "enUS"
    const created = LIB.createUniver({
      locale: type,
      locales: { [type]: LIB.mergeLocales(locale.pack) },
      darkMode: has(el, "dark"),
      presets: [
        LIB.UniverSheetsCorePreset({
          container,
          header: shown(el, "header"),
          toolbar: shown(el, "toolbar"),
          footer: shown(el, "footer"),
          formulaBar: shown(el, "formula-bar"),
          contextMenu: shown(el, "context-menu"),
        }),
      ],
    });
    univer = created.univer;
    const { univerAPI } = created;
    const workbook = univerAPI.createWorkbook(data);
    const readOnly = has(el, "readonly");
    // Univer resets permissions until the Rendered stage, and binds pointer
    // input on the frames after it.
    await whenStage(univerAPI, RENDERED);
    await nextFrames(2);
    if (!live()) return abort(el, token, univer);
    if (readOnly) {
      // setEditable alone does not block typing in Univer 1.0; the
      // permission mode blocks both the UI and the facade API.
      workbook.setEditable(false);
      await workbook.getWorkbookPermission().setReadOnly();
      if (!live()) return abort(el, token, univer);
    }
    const state = {
      univer,
      univerAPI,
      workbook,
      readOnly,
      saveUrl: sameOrigin(attr(el, "save-url")),
      autosave: Number.parseInt(attr(el, "autosave") || "0", 10) || 0,
      version: 0, // edit count
      savedVersion: 0, // edit count of the last good save
      timer: null,
      chain: Promise.resolve(), // saves run one at a time, in order
      queued: null, // a save that waits in the chain
      force: false, // the queued save runs even with no new edits
      subscription: null,
    };
    mounting.delete(el);
    states.set(el, state);
    const unitId = workbook.getId();
    // Count only mutations of this workbook. The cell editor (a doc unit)
    // and the formula engine also send mutations; they change no data.
    state.subscription = univerAPI.addEvent(univerAPI.Event.CommandExecuted, (event) => {
      if (event.type !== MUTATION || state.readOnly) return;
      if (!event.params || event.params.unitId !== unitId) return;
      if (event.options && (event.options.onlyLocal || event.options.fromCollab)) return;
      onChange(el, state);
    });
    if (states.size > 1) activate(active && states.has(active) ? active : el, true);
    setState(el, "ready");
    emit(el, "ready", { univerAPI, workbook });
  } catch (error) {
    if (!live()) return abort(el, token, univer);
    mounting.delete(el);
    freeUniver(univer);
    container.replaceChildren();
    console.error("autumn-univer: mount failed", error);
    setState(el, "error");
    emit(el, "error", { error });
  }
}

// Stops a mount that a dispose or a removal made stale.
function abort(el, token, univer) {
  freeUniver(univer);
  if (mounting.get(el) === token) {
    mounting.delete(el);
    clearMarkers(el);
  }
}

function freeUniver(univer) {
  try {
    if (univer) univer.dispose();
  } catch (error) {
    console.warn("autumn-univer: dispose failed", error);
  }
}

function dirty(state) {
  return state.version !== state.savedVersion;
}

function onChange(el, state) {
  state.version += 1;
  setState(el, "dirty");
  emit(el, "change", { version: state.version });
  if (!state.saveUrl || state.autosave <= 0) return;
  clearTimeout(state.timer);
  state.timer = setTimeout(() => save(el), state.autosave);
}

// POSTs one snapshot. Updates the element only while `state` is its live
// state, so a late answer cannot change a newer sheet.
async function send(el, state, body, version, keepalive = false) {
  const live = () => states.get(el) === state;
  if (live()) setState(el, "saving");
  emit(el, "saving", { version });
  try {
    const response = await fetch(state.saveUrl, {
      method: "POST",
      credentials: "same-origin",
      redirect: "error",
      keepalive,
      headers: { "Content-Type": "application/json", Accept: "application/json", ...csrfHeaders() },
      body,
    });
    if (!response.ok) {
      if (live()) setState(el, "error");
      emit(el, "save-error", { status: response.status, response });
      return false;
    }
    state.savedVersion = Math.max(state.savedVersion, version);
    if (live()) setState(el, dirty(state) ? "dirty" : "saved");
    emit(el, "saved", { status: response.status, version, response });
    return true;
  } catch (error) {
    if (live()) setState(el, "error");
    emit(el, "save-error", { error });
    return false;
  }
}

// Saves after the save in progress, if any. Two calls that wait together
// share one request, which sends the newest data.
function save(el, options = {}) {
  const state = states.get(el);
  if (!state || !state.saveUrl || state.readOnly) return Promise.resolve(false);
  clearTimeout(state.timer);
  state.timer = null;
  if (options.force) state.force = true;
  if (state.queued) return state.queued;
  const run = state.chain.then(() => {
    state.queued = null;
    if (states.get(el) !== state) return false;
    const force = state.force;
    state.force = false;
    if (!dirty(state) && !force) return true;
    return send(el, state, JSON.stringify(state.workbook.save()), state.version);
  });
  state.queued = run;
  state.chain = run.catch(() => false);
  return run;
}

// Frees the Univer instance. Unsaved edits of an autosave sheet go out
// first, after any save in progress.
function dispose(el) {
  if (mounting.has(el)) {
    mounting.delete(el); // The mount sees this and frees its instance.
    clearMarkers(el);
    return;
  }
  const state = states.get(el);
  if (!state) {
    clearMarkers(el);
    return;
  }
  clearTimeout(state.timer);
  if (state.saveUrl && state.autosave > 0 && !state.readOnly && dirty(state)) {
    const body = JSON.stringify(state.workbook.save());
    const version = state.version;
    state.chain = state.chain.then(() => send(el, state, body, version));
  }
  states.delete(el);
  if (active === el) active = null;
  try {
    if (state.subscription) state.subscription.dispose();
  } catch (error) {
    console.warn("autumn-univer: dispose failed", error);
  }
  freeUniver(state.univer);
  clearMarkers(el);
}

// Univer 1.0 gives editor nodes fixed ids (`__editor_…`) and finds them
// with getElementById. With two sheets on one page, keys then go to the
// wrong sheet. So only the active sheet keeps its ids; the other sheets
// get a suffix until the user moves to them.
const INACTIVE = "--autumn-inactive";
let active = null;

function activate(el, force = false) {
  if (active === el && !force) return;
  active = el;
  for (const other of states.keys()) {
    const on = other === el;
    for (const node of other.querySelectorAll("[id]")) {
      const id = node.id;
      if (on && id.endsWith(INACTIVE)) node.id = id.slice(0, -INACTIVE.length);
      else if (!on && !id.endsWith(INACTIVE)) node.id = id + INACTIVE;
    }
  }
}

function onEnter(event) {
  const el = event.target instanceof Element && event.target.closest(SELECTOR);
  if (el && states.has(el)) activate(el);
}

function scan(root) {
  if (!root || (root.nodeType !== Node.ELEMENT_NODE && root.nodeType !== Node.DOCUMENT_NODE)) return;
  if (root.matches && root.matches(SELECTOR)) mount(root);
  root.querySelectorAll(SELECTOR).forEach(mount);
}

// Disposes sheets that left the DOM. It runs one task later, so a sheet
// that a script moves (remove, then insert) stays mounted.
let reapTimer = null;
function scheduleReap() {
  if (reapTimer !== null) return;
  reapTimer = setTimeout(() => {
    reapTimer = null;
    for (const el of [...states.keys(), ...mounting.keys()]) if (!el.isConnected) dispose(el);
  }, 0);
}

document.addEventListener("pointerdown", onEnter, true);
document.addEventListener("focusin", onEnter, true);
document.addEventListener("htmx:load", (event) => scan(event.target));
document.addEventListener("htmx:beforeCleanupElement", (event) => {
  if (states.has(event.target) || mounting.has(event.target)) dispose(event.target);
});
document.addEventListener("click", (event) => {
  const button = event.target instanceof Element && event.target.closest("[data-univer-save-for]");
  if (!button) return;
  const el = document.getElementById(button.getAttribute("data-univer-save-for"));
  if (el) save(el, { force: true });
});
// The page unloads: send unsaved autosave edits with keepalive. Browsers
// cap all keepalive bodies together, so count bytes over all sheets.
window.addEventListener("pagehide", () => {
  let budget = KEEPALIVE_MAX;
  for (const [el, state] of states) {
    if (!state.saveUrl || state.autosave <= 0 || state.readOnly || !dirty(state)) continue;
    clearTimeout(state.timer);
    const body = JSON.stringify(state.workbook.save());
    const size = new Blob([body]).size;
    const keepalive = size <= budget;
    if (keepalive) budget -= size;
    send(el, state, body, state.version, keepalive);
  }
});
// Manual-save sheets with unsaved edits: ask before the page unloads.
window.addEventListener("beforeunload", (event) => {
  for (const state of states.values()) {
    if (state.saveUrl && state.autosave <= 0 && !state.readOnly && dirty(state)) {
      event.preventDefault();
      return;
    }
  }
});
new MutationObserver((records) => {
  for (const record of records) {
    if (record.removedNodes.length > 0) scheduleReap();
    for (const node of record.addedNodes) if (node.nodeType === Node.ELEMENT_NODE) scan(node);
  }
}).observe(document.documentElement, { childList: true, subtree: true });

globalThis.AutumnUniver = Object.freeze({
  mount,
  dispose,
  save: (el) => save(el, { force: true }),
  scan,
  get: (el) => {
    const state = states.get(el);
    return state ? { univerAPI: state.univerAPI, workbook: state.workbook } : undefined;
  },
});

scan(document);
