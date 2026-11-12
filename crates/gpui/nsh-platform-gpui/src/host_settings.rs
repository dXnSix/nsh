//! The embedding app's settings in place of the system's, besides the
//! locale and text size ([`nsh_core::platform::HostEnvironment`]): reduce
//! motion, the screen reader, the time zone and the app's state
//! ([`crate::AppView`]'s `set_reduce_motion`, `set_screen_reader`,
//! `set_time_zone`, `set_lifecycle`). A host whose app has its own setting
//! for one passes it on; on Tizen, where no service in the process reports
//! the TV's settings, they are how the TV host passes on
//! `system_settings`' and the app framework's pause and resume.

use nsh_engine::{Environment, Lifecycle, TimeZone};

/// The settings an embedding app gives a surface (`None`: the system's).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct HostSettings {
    /// In place of [`Environment::reduce_motion`].
    pub(crate) reduce_motion: Option<bool>,
    /// In place of [`Environment::screen_reader`].
    pub(crate) screen_reader: Option<bool>,
    /// In place of the time zone of [`Environment::locale_settings`].
    pub(crate) time_zone: Option<TimeZone>,
    /// In place of [`Environment::lifecycle`] (the windows' activation and
    /// visibility).
    pub(crate) lifecycle: Option<Lifecycle>,
}

impl HostSettings {
    /// Writes the settings `self` has over `env`'s.
    pub(crate) fn apply(&self, env: &mut Environment) {
        if let Some(reduce) = self.reduce_motion {
            env.reduce_motion = reduce;
        }
        if let Some(on) = self.screen_reader {
            env.screen_reader = on;
        }
        if let Some(zone) = self.time_zone {
            env.locale_settings.time_zone = zone;
        }
        if let Some(lifecycle) = self.lifecycle {
            env.lifecycle = lifecycle;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hosts_settings_go_over_the_systems() {
        let system = Environment {
            reduce_motion: true,
            screen_reader: true,
            ..Environment::default()
        };
        // None: the system's.
        let mut env = system;
        HostSettings::default().apply(&mut env);
        assert_eq!(env, system);
        let seoul = TimeZone::parse("Asia/Seoul").unwrap();
        let host = HostSettings {
            reduce_motion: Some(false),
            screen_reader: Some(false),
            time_zone: Some(seoul),
            lifecycle: None,
        };
        host.apply(&mut env);
        assert!(!env.reduce_motion && !env.screen_reader);
        assert_eq!(env.locale_settings.time_zone, seoul);
        // The rest stays the system's.
        assert_eq!(env.locale, system.locale);
        assert_eq!(env.font_scale, system.font_scale);
        // One setting alone.
        let mut env = Environment::default();
        HostSettings {
            reduce_motion: Some(true),
            ..HostSettings::default()
        }
        .apply(&mut env);
        assert!(env.reduce_motion);
        assert_eq!(env.locale_settings.time_zone, TimeZone::UTC);
    }

    /// The embedding app's state of the app over the windows' (the TV
    /// host's pause and resume).
    mod lifecycle {
        use super::*;

        #[test]
        fn the_hosts_state_goes_over_the_windows() {
            let system = Environment {
                lifecycle: Lifecycle::Inactive,
                ..Environment::default()
            };
            let mut env = system;
            HostSettings::default().apply(&mut env);
            assert_eq!(env.lifecycle, Lifecycle::Inactive);
            for state in [Lifecycle::Background, Lifecycle::Active] {
                let mut env = system;
                HostSettings {
                    lifecycle: Some(state),
                    ..HostSettings::default()
                }
                .apply(&mut env);
                assert_eq!(env.lifecycle, state);
                // Nothing else changes.
                assert_eq!(
                    Environment {
                        lifecycle: system.lifecycle,
                        ..env
                    },
                    system
                );
            }
        }
    }
}
