// autumn-plugin-univer: mounts Univer on each [data-univer] element.
//
// Load order: univer.js (sets globalThis.AutumnUniverLib), then this file.
// Both are module scripts, so they run after parsing, in document order.
//
// Lifecycle: mount on page load and on "htmx:load". Dispose on
// "htmx:beforeCleanupElement" and when a mounted element leaves the DOM.
// Events (on the element, they bubble): autumn-univer:ready, change,
// saving, saved, save-error, error.

const LIB = globalThis.AutumnUniverLib;
const PREFIX = "autumn-univer:";
const SELECTOR = "[data-univer]";
const MUTATION = 2; // Univer CommandType.MUTATION
const RENDERED = 2; // Univer LifecycleStages.Rendered
const CSS_LENGTH = /^\d+(\.\d+)?(px|rem|em|vh|vw|%)$/;
const KEEPALIVE_MAX = 60000; // Browsers cap keepalive bodies at 64 KiB.

const states = new Map(); // element -> state

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

async function loadLocale(code) {
  const loader = LIB.locales[code];
  if (!loader) {
    if (code !== "en-US") console.warn(`autumn-univer: locale ${code} is not bundled; using en-US`);
    return code === "en-US" ? Promise.reject(new Error("en-US is missing")) : loadLocale("en-US");
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
  const url = attr(el, "load-url");
  if (!url) return {};
  const response = await fetch(url, { credentials: "same-origin", headers: { Accept: "application/json" } });
  if (!response.ok) throw new Error(`load ${url}: HTTP ${response.status}`);
  return response.json();
}

async function mount(el) {
  if (!LIB) {
    console.error("autumn-univer: univer.js did not load before init.js");
    return;
  }
  if (el.hasAttribute("data-univer-init")) return;
  el.setAttribute("data-univer-init", "");
  setState(el, "loading");
  const height = attr(el, "height");
  if (height && CSS_LENGTH.test(height)) el.style.height = height;
  try {
    const [data, locale] = await Promise.all([loadData(el), loadLocale(attr(el, "locale") || "en-US")]);
    if (!el.isConnected || !el.hasAttribute("data-univer-init")) {
      el.removeAttribute("data-univer-init"); // Removed while loading.
      return;
    }
    const type = locale.code.replace("-", ""); // LocaleType: "en-US" -> "enUS"
    const container = el.querySelector(":scope > [data-univer-mount]") || el;
    const { univer, univerAPI } = LIB.createUniver({
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
    const workbook = univerAPI.createWorkbook(data);
    const readOnly = has(el, "readonly");
    // Univer resets permissions until the Rendered stage.
    await whenStage(univerAPI, RENDERED);
    if (readOnly) {
      // setEditable alone does not block typing in Univer 1.0; the
      // permission mode blocks both the UI and the facade API.
      workbook.setEditable(false);
      await workbook.getWorkbookPermission().setReadOnly();
    }
    const state = {
      univer,
      univerAPI,
      workbook,
      readOnly,
      saveUrl: attr(el, "save-url"),
      autosave: Number.parseInt(attr(el, "autosave") || "0", 10) || 0,
      version: 0, // edit count
      savedVersion: 0, // edit count of the last good save
      timer: null,
      inFlight: null,
      subscription: null,
    };
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
    setState(el, "ready");
    emit(el, "ready", { univerAPI, workbook });
  } catch (error) {
    console.error("autumn-univer: mount failed", error);
    setState(el, "error");
    emit(el, "error", { error });
  }
}

function onChange(el, state) {
  state.version += 1;
  setState(el, "dirty");
  emit(el, "change", { version: state.version });
  if (!state.saveUrl || state.autosave <= 0) return;
  clearTimeout(state.timer);
  state.timer = setTimeout(() => save(el), state.autosave);
}

async function save(el, options = {}) {
  const state = states.get(el);
  if (!state || !state.saveUrl || state.readOnly) return false;
  clearTimeout(state.timer);
  state.timer = null;
  // One save at a time. A save asked for during a save runs after it.
  if (state.inFlight) {
    await state.inFlight.catch(() => {});
    if (states.get(el) !== state) return false;
  }
  if (state.version === state.savedVersion && !options.force) return true;
  const version = state.version;
  const body = JSON.stringify(state.workbook.save());
  setState(el, "saving");
  emit(el, "saving", { version });
  state.inFlight = fetch(state.saveUrl, {
    method: "POST",
    credentials: "same-origin",
    keepalive: Boolean(options.keepalive) && body.length < KEEPALIVE_MAX,
    headers: { "Content-Type": "application/json", Accept: "application/json", ...csrfHeaders() },
    body,
  });
  try {
    const response = await state.inFlight;
    if (!response.ok) {
      setState(el, "error");
      emit(el, "save-error", { status: response.status, response });
      return false;
    }
    state.savedVersion = Math.max(state.savedVersion, version);
    setState(el, state.version === state.savedVersion ? "saved" : "dirty");
    emit(el, "saved", { status: response.status, version, response });
    return true;
  } catch (error) {
    setState(el, "error");
    emit(el, "save-error", { error });
    return false;
  } finally {
    state.inFlight = null;
  }
}

// Saves unsaved edits (best effort), then frees the Univer instance.
function dispose(el) {
  const state = states.get(el);
  if (!state) {
    el.removeAttribute("data-univer-init");
    return;
  }
  if (state.saveUrl && state.autosave > 0 && state.version !== state.savedVersion) {
    save(el, { keepalive: true });
  }
  states.delete(el);
  clearTimeout(state.timer);
  try {
    if (state.subscription) state.subscription.dispose();
    state.univer.dispose();
  } catch (error) {
    console.warn("autumn-univer: dispose failed", error);
  }
  el.removeAttribute("data-univer-init");
  el.removeAttribute("data-univer-state");
}

function scan(root) {
  if (!root || root.nodeType !== Node.ELEMENT_NODE && root.nodeType !== Node.DOCUMENT_NODE) return;
  if (root.matches && root.matches(SELECTOR)) mount(root);
  root.querySelectorAll(SELECTOR).forEach(mount);
}

function reap() {
  for (const el of [...states.keys()]) if (!el.isConnected) dispose(el);
}

document.addEventListener("htmx:load", (event) => scan(event.target));
document.addEventListener("htmx:beforeCleanupElement", (event) => {
  if (states.has(event.target)) dispose(event.target);
});
document.addEventListener("click", (event) => {
  const button = event.target instanceof Element && event.target.closest("[data-univer-save-for]");
  if (!button) return;
  const el = document.getElementById(button.getAttribute("data-univer-save-for"));
  if (el) save(el, { force: true });
});
window.addEventListener("pagehide", () => {
  for (const [el, state] of states) {
    if (state.saveUrl && state.autosave > 0 && state.version !== state.savedVersion) save(el, { keepalive: true });
  }
});
new MutationObserver((records) => {
  if (records.some((r) => r.removedNodes.length > 0)) reap();
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
