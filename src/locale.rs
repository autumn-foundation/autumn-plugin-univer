//! [`Locale`]: the UI language of a spreadsheet.
//!
//! Each locale pack is a lazy chunk. The browser loads only the pack that
//! the page needs. `en-US` is always in the bundle. Every other locale needs
//! its Cargo feature (`locale-fr-fr`, …) or `all-locales`. So a variant
//! exists only when its pack is in the bundle.

use std::fmt;
use std::str::FromStr;

/// The UI language of a [`Spreadsheet`](crate::Spreadsheet).
///
/// ```rust
/// use autumn_plugin_univer::Locale;
///
/// assert_eq!(Locale::default(), Locale::EnUs);
/// assert_eq!(Locale::EnUs.code(), "en-US");
/// assert_eq!("en-US".parse::<Locale>(), Ok(Locale::EnUs));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Locale {
    /// Arabic (Saudi Arabia). `ar-SA`. Needs the `locale-ar-sa` feature.
    #[cfg(feature = "locale-ar-sa")]
    ArSa,
    /// Catalan (Spain). `ca-ES`. Needs the `locale-ca-es` feature.
    #[cfg(feature = "locale-ca-es")]
    CaEs,
    /// German (Germany). `de-DE`. Needs the `locale-de-de` feature.
    #[cfg(feature = "locale-de-de")]
    DeDe,
    /// English (United States). `en-US`.
    #[default]
    EnUs,
    /// Spanish (Spain). `es-ES`. Needs the `locale-es-es` feature.
    #[cfg(feature = "locale-es-es")]
    EsEs,
    /// Persian (Iran). `fa-IR`. Needs the `locale-fa-ir` feature.
    #[cfg(feature = "locale-fa-ir")]
    FaIr,
    /// French (France). `fr-FR`. Needs the `locale-fr-fr` feature.
    #[cfg(feature = "locale-fr-fr")]
    FrFr,
    /// Indonesian (Indonesia). `id-ID`. Needs the `locale-id-id` feature.
    #[cfg(feature = "locale-id-id")]
    IdId,
    /// Italian (Italy). `it-IT`. Needs the `locale-it-it` feature.
    #[cfg(feature = "locale-it-it")]
    ItIt,
    /// Japanese (Japan). `ja-JP`. Needs the `locale-ja-jp` feature.
    #[cfg(feature = "locale-ja-jp")]
    JaJp,
    /// Korean (Korea). `ko-KR`. Needs the `locale-ko-kr` feature.
    #[cfg(feature = "locale-ko-kr")]
    KoKr,
    /// Polish (Poland). `pl-PL`. Needs the `locale-pl-pl` feature.
    #[cfg(feature = "locale-pl-pl")]
    PlPl,
    /// Portuguese (Brazil). `pt-BR`. Needs the `locale-pt-br` feature.
    #[cfg(feature = "locale-pt-br")]
    PtBr,
    /// Russian (Russia). `ru-RU`. Needs the `locale-ru-ru` feature.
    #[cfg(feature = "locale-ru-ru")]
    RuRu,
    /// Slovak (Slovakia). `sk-SK`. Needs the `locale-sk-sk` feature.
    #[cfg(feature = "locale-sk-sk")]
    SkSk,
    /// Vietnamese (Vietnam). `vi-VN`. Needs the `locale-vi-vn` feature.
    #[cfg(feature = "locale-vi-vn")]
    ViVn,
    /// Chinese (Simplified, China). `zh-CN`. Needs the `locale-zh-cn` feature.
    #[cfg(feature = "locale-zh-cn")]
    ZhCn,
    /// Chinese (Traditional, Hong Kong). `zh-HK`. Needs the `locale-zh-hk` feature.
    #[cfg(feature = "locale-zh-hk")]
    ZhHk,
    /// Chinese (Traditional, Taiwan). `zh-TW`. Needs the `locale-zh-tw` feature.
    #[cfg(feature = "locale-zh-tw")]
    ZhTw,
}

/// Every compiled-in locale, in code order.
const ALL: &[Locale] = &[
    #[cfg(feature = "locale-ar-sa")]
    Locale::ArSa,
    #[cfg(feature = "locale-ca-es")]
    Locale::CaEs,
    #[cfg(feature = "locale-de-de")]
    Locale::DeDe,
    Locale::EnUs,
    #[cfg(feature = "locale-es-es")]
    Locale::EsEs,
    #[cfg(feature = "locale-fa-ir")]
    Locale::FaIr,
    #[cfg(feature = "locale-fr-fr")]
    Locale::FrFr,
    #[cfg(feature = "locale-id-id")]
    Locale::IdId,
    #[cfg(feature = "locale-it-it")]
    Locale::ItIt,
    #[cfg(feature = "locale-ja-jp")]
    Locale::JaJp,
    #[cfg(feature = "locale-ko-kr")]
    Locale::KoKr,
    #[cfg(feature = "locale-pl-pl")]
    Locale::PlPl,
    #[cfg(feature = "locale-pt-br")]
    Locale::PtBr,
    #[cfg(feature = "locale-ru-ru")]
    Locale::RuRu,
    #[cfg(feature = "locale-sk-sk")]
    Locale::SkSk,
    #[cfg(feature = "locale-vi-vn")]
    Locale::ViVn,
    #[cfg(feature = "locale-zh-cn")]
    Locale::ZhCn,
    #[cfg(feature = "locale-zh-hk")]
    Locale::ZhHk,
    #[cfg(feature = "locale-zh-tw")]
    Locale::ZhTw,
];

impl Locale {
    /// The BCP 47 code, for example `en-US`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            #[cfg(feature = "locale-ar-sa")]
            Self::ArSa => "ar-SA",
            #[cfg(feature = "locale-ca-es")]
            Self::CaEs => "ca-ES",
            #[cfg(feature = "locale-de-de")]
            Self::DeDe => "de-DE",
            Self::EnUs => "en-US",
            #[cfg(feature = "locale-es-es")]
            Self::EsEs => "es-ES",
            #[cfg(feature = "locale-fa-ir")]
            Self::FaIr => "fa-IR",
            #[cfg(feature = "locale-fr-fr")]
            Self::FrFr => "fr-FR",
            #[cfg(feature = "locale-id-id")]
            Self::IdId => "id-ID",
            #[cfg(feature = "locale-it-it")]
            Self::ItIt => "it-IT",
            #[cfg(feature = "locale-ja-jp")]
            Self::JaJp => "ja-JP",
            #[cfg(feature = "locale-ko-kr")]
            Self::KoKr => "ko-KR",
            #[cfg(feature = "locale-pl-pl")]
            Self::PlPl => "pl-PL",
            #[cfg(feature = "locale-pt-br")]
            Self::PtBr => "pt-BR",
            #[cfg(feature = "locale-ru-ru")]
            Self::RuRu => "ru-RU",
            #[cfg(feature = "locale-sk-sk")]
            Self::SkSk => "sk-SK",
            #[cfg(feature = "locale-vi-vn")]
            Self::ViVn => "vi-VN",
            #[cfg(feature = "locale-zh-cn")]
            Self::ZhCn => "zh-CN",
            #[cfg(feature = "locale-zh-hk")]
            Self::ZhHk => "zh-HK",
            #[cfg(feature = "locale-zh-tw")]
            Self::ZhTw => "zh-TW",
        }
    }

    /// Every locale in this build, in code order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        ALL
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// The error for a locale code that is not in this build.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("locale `{0}` is not in this build (enable its `locale-*` feature)")]
pub struct UnknownLocale(pub String);

impl FromStr for Locale {
    type Err = UnknownLocale;

    /// Parses a BCP 47 code. The match ignores case and accepts `_`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let wanted = s.replace('_', "-");
        ALL.iter()
            .copied()
            .find(|l| l.code().eq_ignore_ascii_case(&wanted))
            .ok_or_else(|| UnknownLocale(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::UNIVER_ASSETS;

    #[test]
    fn default_is_en_us() {
        assert_eq!(Locale::default(), Locale::EnUs);
        assert_eq!(Locale::EnUs.to_string(), "en-US");
    }

    #[test]
    fn codes_round_trip() {
        for &l in Locale::all() {
            assert_eq!(l.code().parse::<Locale>(), Ok(l));
            assert_eq!(l.code().to_lowercase().replace('-', "_").parse::<Locale>(), Ok(l));
        }
    }

    #[test]
    fn all_is_sorted_and_unique() {
        let codes: Vec<&str> = Locale::all().iter().map(|l| l.code()).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(codes, sorted);
    }

    #[test]
    fn every_compiled_locale_has_its_chunk() {
        for &l in Locale::all() {
            let prefix = format!("chunks/{}-", l.code());
            assert!(
                UNIVER_ASSETS.iter().any(|a| a.logical_path().starts_with(&prefix)),
                "{l} has a bundled chunk"
            );
        }
    }

    #[test]
    fn unknown_codes_are_errors() {
        assert_eq!(
            "xx-XX".parse::<Locale>(),
            Err(UnknownLocale("xx-XX".to_owned()))
        );
        assert!(UnknownLocale("xx".into()).to_string().contains("xx"));
    }

    #[cfg(feature = "all-locales")]
    #[test]
    fn all_locales_builds_hold_nineteen_locales() {
        assert_eq!(Locale::all().len(), 19);
    }
}
