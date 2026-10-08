// Entry point of the vendored Univer bundle.
// It exposes the Univer API that `assets/init.js` uses as one global.
// Locale packs are lazy chunks: the browser loads only the one it needs.
import { createUniver, LocaleType, mergeLocales } from "@univerjs/presets";
import { UniverSheetsCorePreset } from "@univerjs/preset-sheets-core";

globalThis.AutumnUniverLib = Object.freeze({
  createUniver,
  LocaleType,
  mergeLocales,
  UniverSheetsCorePreset,
  locales: Object.freeze({
    "ar-SA": () => import("@univerjs/preset-sheets-core/locales/ar-SA"),
    "ca-ES": () => import("@univerjs/preset-sheets-core/locales/ca-ES"),
    "de-DE": () => import("@univerjs/preset-sheets-core/locales/de-DE"),
    "en-US": () => import("@univerjs/preset-sheets-core/locales/en-US"),
    "es-ES": () => import("@univerjs/preset-sheets-core/locales/es-ES"),
    "fa-IR": () => import("@univerjs/preset-sheets-core/locales/fa-IR"),
    "fr-FR": () => import("@univerjs/preset-sheets-core/locales/fr-FR"),
    "id-ID": () => import("@univerjs/preset-sheets-core/locales/id-ID"),
    "it-IT": () => import("@univerjs/preset-sheets-core/locales/it-IT"),
    "ja-JP": () => import("@univerjs/preset-sheets-core/locales/ja-JP"),
    "ko-KR": () => import("@univerjs/preset-sheets-core/locales/ko-KR"),
    "pl-PL": () => import("@univerjs/preset-sheets-core/locales/pl-PL"),
    "pt-BR": () => import("@univerjs/preset-sheets-core/locales/pt-BR"),
    "ru-RU": () => import("@univerjs/preset-sheets-core/locales/ru-RU"),
    "sk-SK": () => import("@univerjs/preset-sheets-core/locales/sk-SK"),
    "vi-VN": () => import("@univerjs/preset-sheets-core/locales/vi-VN"),
    "zh-CN": () => import("@univerjs/preset-sheets-core/locales/zh-CN"),
    "zh-HK": () => import("@univerjs/preset-sheets-core/locales/zh-HK"),
    "zh-TW": () => import("@univerjs/preset-sheets-core/locales/zh-TW"),
  }),
});
