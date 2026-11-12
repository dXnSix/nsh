//! The environment as JS reads it (`platform`), one group per area:
//! `cargo test -p nsh-js --lib env::tests::<group>`.

/// The TV category: `platform.tv` in the TV idiom on any OS (never
/// desktop), `tv={{…}}` props there only, and `platform.select`'s `tv`
/// key after the OS's own and before `desktop` / `mobile`.
mod tv {
    use nsh_core::{
        Op, Prop,
        platform::{Environment, Idiom, Os},
    };

    use crate::host::Host;

    /// Reports `platform` as one text, then sets `testID`s through the
    /// `tv` and `desktop` groups of one view, the `tv` one bound to
    /// `globalThis.group` (a signal).
    const APP: &str = r#"
        import { element, text, prop, bindProp } from "nsh:rt";
        import { platform, signal } from "nsh";
        const pick = (spec) => platform.select(spec) ?? "-";
        text([
            platform.os, platform.idiom, platform.tv, platform.desktop,
            pick({ [platform.os]: "os", tv: "tv", desktop: "desktop", mobile: "mobile" }),
            pick({ tv: "tv", desktop: "desktop", mobile: "mobile" }),
            pick({ desktop: "desktop", mobile: "mobile" }),
            pick({ tv: "tv" }),
            pick({ default: "default" }),
        ].join(" "));
        globalThis.group = signal({ testID: "tv", nativeID: "on a TV" });
        const view = element("view");
        bindProp(view, "tv", () => group());
        const other = element("view");
        prop(other, "desktop", { testID: "desktop" });
    "#;

    /// What the app showed on `os` in `idiom`: its report, then the test
    /// ids and native ids its views were given (in order).
    pub(super) fn run(os: Os, idiom: Idiom) -> (Host, String, Vec<String>) {
        let host = Host::new().unwrap();
        host.set_environment(&Environment {
            os,
            idiom,
            ..Environment::default()
        });
        host.eval_source(APP).unwrap();
        let (report, props) = seen(&host);
        (host, report.join("|"), props)
    }

    /// The texts and the props (`testID=…`, `nativeID=…`) sent since the last
    /// call.
    fn seen(host: &Host) -> (Vec<String>, Vec<String>) {
        let mut ops = Vec::new();
        host.take_ops(&mut ops);
        let mut texts = Vec::new();
        let mut props = Vec::new();
        for op in ops {
            match op {
                Op::CreateText { text, .. } => texts.push(text),
                Op::SetProp {
                    prop: Prop::TestId(id),
                    ..
                } => props.push(format!("testID={}", id.as_deref().unwrap_or("-"))),
                Op::SetProp {
                    prop: Prop::NativeId(id),
                    ..
                } => props.push(format!("nativeID={}", id.as_deref().unwrap_or("-"))),
                _ => {}
            }
        }
        (texts, props)
    }

    #[test]
    fn platform_tv_is_the_tv_idiom_on_any_os_and_never_desktop() {
        for os in [Os::Android, Os::Web, Os::Ios, Os::MacOs, Os::Linux] {
            let (_, report, _) = run(os, Idiom::Tv);
            assert_eq!(
                report,
                format!("{} tv true false os tv mobile tv default", os.name()),
                "{os:?}"
            );
        }
        // Not a TV: `tv` is false and the `tv` key is passed over.
        let (_, report, _) = run(Os::Android, Idiom::Phone);
        assert_eq!(
            report,
            "android phone false false os mobile mobile - default"
        );
        let (_, report, _) = run(Os::MacOs, Idiom::Desktop);
        assert_eq!(
            report,
            "macos desktop false true os desktop desktop - default"
        );
        let (_, report, _) = run(Os::Web, Idiom::Pad);
        assert_eq!(report, "web pad false false os mobile mobile - default");
    }

    #[test]
    fn tv_props_apply_on_a_tv_only() {
        let (_, _, props) = run(Os::Web, Idiom::Tv);
        // On a TV, the `tv` group's props, and not the desktop's.
        assert_eq!(props, ["testID=tv", "nativeID=on a TV"]);
        let (_, _, props) = run(Os::Android, Idiom::Tv);
        assert_eq!(props, ["testID=tv", "nativeID=on a TV"]);
        // A desktop applies its own group, not the TV's.
        let (_, _, props) = run(Os::MacOs, Idiom::Desktop);
        assert_eq!(props, ["testID=desktop"]);
        // A phone applies neither.
        let (_, _, props) = run(Os::Android, Idiom::Phone);
        assert!(props.is_empty(), "{props:?}");
    }

    #[test]
    fn a_key_the_tv_group_no_longer_has_is_cleared() {
        let (host, _, _) = run(Os::Android, Idiom::Tv);
        host.eval_source(r#"globalThis.group.set({ testID: "tv 2" });"#)
            .unwrap();
        let (_, props) = seen(&host);
        assert_eq!(props, ["nativeID=-", "testID=tv 2"]);
        host.eval_source("globalThis.group.set(null);").unwrap();
        let (_, props) = seen(&host);
        assert_eq!(props, ["testID=-"]);
        // Off a TV the group's changes do nothing.
        let (host, _, _) = run(Os::Android, Idiom::Phone);
        host.eval_source(r#"globalThis.group.set({ testID: "tv 2" });"#)
            .unwrap();
        assert!(seen(&host).1.is_empty());
    }
}

/// Tizen natively (`Os::Tizen`): `platform.os` is `"tizen"` in the TV
/// idiom, never desktop, and `platform.select` takes its `tizen` key before
/// `tv`; a Tizen web app is `"web"` and takes `web`'s.
mod tizen {
    use nsh_core::{
        Op,
        platform::{Environment, Idiom, Os},
    };

    use crate::host::Host;

    /// Reports `platform` and what `platform.select` picks, as one text.
    const APP: &str = r#"
        import { text } from "nsh:rt";
        import { platform } from "nsh";
        const pick = (spec) => platform.select(spec) ?? "-";
        text([
            platform.name, platform.os, platform.os === "tizen", platform.idiom,
            platform.tv, platform.desktop,
            pick({ tizen: "tizen", web: "web", tv: "tv", mobile: "mobile" }),
            pick({ linux: "linux", tv: "tv", desktop: "desktop" }),
            pick({ native: "native", default: "default" }),
        ].join(" "));
    "#;

    /// What the app showed on `os` in `idiom`.
    fn report(os: Os, idiom: Idiom) -> String {
        let host = Host::new().unwrap();
        host.set_environment(&Environment {
            os,
            idiom,
            ..Environment::default()
        });
        host.eval_source(APP).unwrap();
        let mut ops = Vec::new();
        host.take_ops(&mut ops);
        ops.into_iter()
            .filter_map(|op| match op {
                Op::CreateText { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn tizen_reports_its_os_and_its_select_key() {
        assert_eq!(
            report(Os::Tizen, Idiom::default_on(Os::Tizen)),
            "tizen tizen true tv true false tizen tv native"
        );
    }

    #[test]
    fn tizen_is_never_a_desktop() {
        // Not Linux's key, and no desktop, whatever idiom was set: one it
        // does not apply in reads as Tizen's own, the TV.
        assert_eq!(
            report(Os::Tizen, Idiom::Desktop),
            "tizen tizen true tv true false tizen tv native"
        );
        assert_eq!(
            report(Os::Tizen, Idiom::Phone),
            "tizen tizen true phone false false tizen - native"
        );
    }

    #[test]
    fn tizen_web_apps_are_web_on_a_tv() {
        assert_eq!(
            report(Os::Web, Idiom::Tv),
            "web web false tv true false web tv default"
        );
    }

    #[test]
    fn tizen_takes_tv_props() {
        let (_, _, props) = super::tv::run(Os::Tizen, Idiom::Tv);
        assert_eq!(props, ["testID=tv", "nativeID=on a TV"]);
    }
}
