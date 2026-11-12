//! The TV's settings for `AppView`: what `dxn_gpui_tizen::settings` reads
//! from `system_settings` (at start and after every change) becomes the
//! values of `AppView`'s setters (`set_locale`, `set_time_zone`,
//! `set_font_scale`, `set_reduce_motion`, `set_screen_reader`,
//! `set_appearance`), which stand in for the system's: nothing else in a
//! TV's process reports them. Only what changed is set again.
//!
//! A setting the TV does not report is `None`, and `AppView` keeps its own
//! default for it (motion on, no screen reader, the process's locale and
//! time zone). Tizen 8.0 reports no light or dark setting: the appearance is
//! dark, the TV UI's (and the Tizen platform's window appearance).

use dxn_gpui_tizen::settings::SystemSettings;
use nsh_platform_gpui::{Appearance, Locale, TimeZone};

/// The values `AppView`'s setters take, from the TV's settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HostValues {
    pub(crate) locale: Option<Locale>,
    pub(crate) time_zone: Option<TimeZone>,
    pub(crate) font_scale: Option<f32>,
    pub(crate) reduce_motion: Option<bool>,
    pub(crate) screen_reader: Option<bool>,
    pub(crate) appearance: Appearance,
}

/// Font scales outside this range are not the TV's (its sizes are 0.8 to
/// 2.0): `AppView` keeps the default.
const FONT_SCALES: std::ops::RangeInclusive<f32> = 0.5..=3.0;

impl HostValues {
    /// The values of `settings`: a locale or zone NSH cannot read, or a
    /// font scale out of range, is `None` (the default stays).
    pub(crate) fn of(settings: &SystemSettings) -> HostValues {
        HostValues {
            locale: settings.locale.as_deref().and_then(Locale::parse),
            time_zone: settings.time_zone.as_deref().and_then(TimeZone::parse),
            font_scale: settings
                .font_scale
                .filter(|scale| scale.is_finite() && FONT_SCALES.contains(scale)),
            reduce_motion: settings.reduce_motion,
            screen_reader: settings.screen_reader,
            appearance: Appearance::Dark,
        }
    }
}

/// One of `AppView`'s setters and its value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Setter {
    Locale(Option<Locale>),
    TimeZone(Option<TimeZone>),
    FontScale(Option<f32>),
    ReduceMotion(Option<bool>),
    ScreenReader(Option<bool>),
    Appearance(Appearance),
}

/// The setters to call for `new`: all of them the first time (`old` is
/// `None`), then those whose value changed.
pub(crate) fn changes(old: Option<&HostValues>, new: &HostValues) -> Vec<Setter> {
    let all = [
        Setter::Locale(new.locale),
        Setter::TimeZone(new.time_zone),
        Setter::FontScale(new.font_scale),
        Setter::ReduceMotion(new.reduce_motion),
        Setter::ScreenReader(new.screen_reader),
        Setter::Appearance(new.appearance),
    ];
    let Some(old) = old else {
        return all.to_vec();
    };
    let before = [
        Setter::Locale(old.locale),
        Setter::TimeZone(old.time_zone),
        Setter::FontScale(old.font_scale),
        Setter::ReduceMotion(old.reduce_motion),
        Setter::ScreenReader(old.screen_reader),
        Setter::Appearance(old.appearance),
    ];
    all.into_iter()
        .zip(before)
        .filter(|(now, was)| now != was)
        .map(|(now, _)| now)
        .collect()
}

/// Calls `setter` on `view`.
pub(crate) fn apply(
    view: &nsh_platform_gpui::AppView,
    setter: Setter,
    cx: &mut gpui::Context<nsh_platform_gpui::AppView>,
) {
    match setter {
        Setter::Locale(locale) => view.set_locale(locale, cx),
        Setter::TimeZone(zone) => view.set_time_zone(zone, cx),
        Setter::FontScale(scale) => view.set_font_scale(scale, cx),
        Setter::ReduceMotion(reduce) => view.set_reduce_motion(reduce, cx),
        Setter::ScreenReader(on) => view.set_screen_reader(on, cx),
        Setter::Appearance(appearance) => view.set_appearance(appearance, cx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn korean_tv() -> SystemSettings {
        SystemSettings {
            locale: Some("ko-KR".into()),
            time_zone: Some("Asia/Seoul".into()),
            font_scale: Some(1.25),
            screen_reader: Some(true),
            high_contrast: None,
            reduce_motion: None,
        }
    }

    #[test]
    fn the_tvs_settings_are_appviews_values() {
        let values = HostValues::of(&korean_tv());
        assert_eq!(values.locale, Locale::parse("ko-KR"));
        assert_eq!(values.time_zone, TimeZone::parse("Asia/Seoul"));
        assert_eq!(values.font_scale, Some(1.25));
        assert_eq!(values.screen_reader, Some(true));
        // Not reported: AppView's default stays.
        assert_eq!(values.reduce_motion, None);
        assert_eq!(values.appearance, Appearance::Dark);
        // Reported by a TV that has the setting.
        let values = HostValues::of(&SystemSettings {
            reduce_motion: Some(true),
            ..korean_tv()
        });
        assert_eq!(values.reduce_motion, Some(true));
    }

    #[test]
    fn what_nsh_cannot_read_keeps_the_default() {
        let values = HostValues::of(&SystemSettings {
            locale: Some("C".into()),
            time_zone: Some("".into()),
            font_scale: Some(f32::NAN),
            ..korean_tv()
        });
        assert_eq!(values.locale, None);
        assert_eq!(values.time_zone, None);
        assert_eq!(values.font_scale, None);
        assert_eq!(
            HostValues::of(&SystemSettings {
                font_scale: Some(9.0),
                ..korean_tv()
            })
            .font_scale,
            None
        );
        // Nothing reported at all (off Tizen, or before the platform read
        // them): every value AppView's own, the appearance the TV's.
        let none = HostValues::of(&SystemSettings::default());
        assert_eq!(
            none,
            HostValues {
                locale: None,
                time_zone: None,
                font_scale: None,
                reduce_motion: None,
                screen_reader: None,
                appearance: Appearance::Dark,
            }
        );
    }

    #[test]
    fn every_setter_first_then_only_what_changed() {
        let first = HostValues::of(&korean_tv());
        let all = changes(None, &first);
        assert_eq!(all.len(), 6);
        assert!(all.contains(&Setter::Locale(Locale::parse("ko-KR"))));
        assert!(all.contains(&Setter::Appearance(Appearance::Dark)));
        assert_eq!(changes(Some(&first), &first), Vec::new());
        let english = HostValues::of(&SystemSettings {
            locale: Some("en-GB".into()),
            font_scale: Some(2.0),
            ..korean_tv()
        });
        assert_eq!(
            changes(Some(&first), &english),
            vec![
                Setter::Locale(Locale::parse("en-GB")),
                Setter::FontScale(Some(2.0)),
            ]
        );
        // A setting the TV stops reporting goes back to the default.
        let unreported = HostValues::of(&SystemSettings {
            screen_reader: None,
            ..korean_tv()
        });
        assert_eq!(
            changes(Some(&first), &unreported),
            vec![Setter::ScreenReader(None)]
        );
    }
}
