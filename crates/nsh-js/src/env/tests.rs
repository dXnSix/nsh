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
    fn run(os: Os, idiom: Idiom) -> (Host, String, Vec<String>) {
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
