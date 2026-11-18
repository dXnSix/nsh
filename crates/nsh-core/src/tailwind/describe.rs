//! What a class token sets ([`describe`]): for a properties panel's
//! dropdowns and for writing an edit back to source, which needs to know
//! which classes of a node a change replaces. It parses the token exactly
//! as the node's classes are parsed.

use super::{Theme, Token, parse_token, split_variants, utilities::Decl};
use crate::{StyleDecl, StyleKey};

/// What one class token does.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassInfo {
    /// Its variants, outermost first (`dark`, `hover`, `md`, ...).
    pub variants: Vec<String>,
    /// The utility after the variants, without `!`.
    pub utility: String,
    /// `!p-4` or `p-4!`: applied after every other rule.
    pub important: bool,
    /// What it sets, in its declarations' order (none for `group` and the
    /// utilities that do nothing natively).
    pub targets: Vec<ClassTarget>,
    /// It names theme tokens (`bg-primary`): its value follows the theme
    /// and the appearance.
    pub reads_theme: bool,
    /// The design tokens it reads, by name, sorted: `bg-primary` reads
    /// `primary`, `rounded-card` `radius-card`, a role's variable
    /// (`bg-(--popover)`) the token the role is mapped onto. What an editor
    /// shows as a class's tokens, and what a change of them restyles.
    pub tokens: Vec<String>,
}

/// One thing a class sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassTarget {
    /// A style property.
    Style(StyleKey),
    /// A step of the `transform` property.
    Transform,
    /// A part of a composed value: a shadow, a ring, a filter, a gradient
    /// stop, a transform's part (`ShadowColor`, `RingWidth`, `From`, ...).
    Part(String),
    /// A node property a class sets: `noWrap`, `caretColor`, `spaceX`,
    /// `spaceY`, `spaceXReverse`, `spaceYReverse`.
    Prop(&'static str),
}

/// What `token` (one class, variants included: `md:hover:p-4!`) does with
/// `theme`; `None` for a token the parser rejects.
pub fn describe(theme: &Theme, token: &str) -> Option<ClassInfo> {
    let parts = split_variants(token);
    let (utility, variants) = parts.split_last()?;
    let mut important = false;
    let mut utility = *utility;
    if let Some(u) = utility.strip_prefix('!') {
        utility = u;
        important = true;
    } else if let Some(u) = utility.strip_suffix('!') {
        utility = u;
        important = true;
    }
    let mut read = Vec::new();
    let (targets, reads_theme) = match parse_token(theme, token, &mut read)? {
        Token::Rule(rule) => (targets(&rule.decls), false),
        Token::Themed(rule, dark) => {
            let mut t = targets(&rule.decls);
            for target in dark.map(|d| targets(&d.decls)).unwrap_or_default() {
                if !t.contains(&target) {
                    t.push(target);
                }
            }
            (t, true)
        }
        Token::Group(_) | Token::Ignored => (Vec::new(), false),
    };
    // The class keeps the tokens it reads by key; the theme has their names.
    let mut tokens: Vec<String> = theme
        .tokens()
        .names()
        .filter(|name| read.contains(&super::theme::token_key(name)))
        .map(str::to_owned)
        .collect();
    tokens.sort_unstable();
    tokens.dedup();
    Some(ClassInfo {
        variants: variants.iter().map(|v| (*v).to_owned()).collect(),
        utility: utility.to_owned(),
        important,
        targets,
        reads_theme,
        tokens,
    })
}

/// The name of a part: its variant's (`ShadowColor`).
fn part_name(part: &impl std::fmt::Debug) -> String {
    let text = format!("{part:?}");
    text.split(['(', ' ', '{'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn targets(decls: &[Decl]) -> Vec<ClassTarget> {
    use super::utilities::PropDecl;
    let mut out = Vec::new();
    for decl in decls {
        let target = match decl {
            Decl::Set(key, _)
            | Decl::Default(key, _)
            | Decl::Screen(key, ..)
            | Decl::Adaptive(key, _)
            | Decl::CurrentColor(key)
            | Decl::SetCurrentColor(key, _)
            | Decl::Ch(key, _) => ClassTarget::Style(*key),
            Decl::Style(style) => match style {
                StyleDecl::Set(key, _)
                | StyleDecl::Default(key, _)
                | StyleDecl::CurrentColor(key) => ClassTarget::Style(*key),
                StyleDecl::SetTransform(..) | StyleDecl::Transform(..) => ClassTarget::Transform,
                StyleDecl::Animated(..) => ClassTarget::Part("Animated".to_owned()),
            },
            Decl::Part(part) => ClassTarget::Part(part_name(part)),
            Decl::AdaptivePart(pair) => ClassTarget::Part(part_name(&pair[0])),
            Decl::CurrentPart(slot, _) => ClassTarget::Part(format!("{}Color", part_name(slot))),
            Decl::Prop(prop) => ClassTarget::Prop(match prop {
                PropDecl::NoWrap(_) => "noWrap",
                PropDecl::Caret(_) => "caretColor",
                PropDecl::Space(false, _) => "spaceX",
                PropDecl::Space(true, _) => "spaceY",
                PropDecl::SpaceReverse(false) => "spaceXReverse",
                PropDecl::SpaceReverse(true) => "spaceYReverse",
            }),
        };
        if !out.contains(&target) {
            out.push(target);
        }
    }
    out
}

#[cfg(test)]
mod describe_tests {
    use super::{ClassTarget, describe};
    use crate::{StyleKey as K, tailwind::Theme};

    fn themed() -> Theme {
        let mut theme = Theme::default();
        theme.set_tokens(&crate::element::tokens::sample());
        theme
    }

    #[test]
    fn variants_utility_importance_and_style_keys() {
        let theme = Theme::default();
        let info = describe(&theme, "md:hover:p-4!").unwrap();
        assert_eq!(info.variants, ["md", "hover"]);
        assert_eq!(info.utility, "p-4");
        assert!(info.important);
        assert_eq!(info.targets, [ClassTarget::Style(K::Padding)]);
        assert!(!info.reads_theme);
        let info = describe(&theme, "!mx-2").unwrap();
        assert!(info.important && info.utility == "mx-2");
        assert_eq!(info.targets, [ClassTarget::Style(K::MarginHorizontal)]);
        // `flex` sets the display and implies a row.
        let flex = describe(&theme, "flex").unwrap().targets;
        assert!(flex.contains(&ClassTarget::Style(K::Display)));
        assert!(flex.contains(&ClassTarget::Style(K::FlexDirection)));
    }

    #[test]
    fn parts_props_and_the_theme() {
        let theme = themed();
        let shadow = describe(&theme, "shadow-md").unwrap();
        assert!(
            shadow
                .targets
                .iter()
                .any(|t| matches!(t, ClassTarget::Part(_))),
            "{shadow:?}"
        );
        assert_eq!(
            describe(&theme, "whitespace-nowrap").unwrap().targets,
            [ClassTarget::Prop("noWrap")]
        );
        assert_eq!(
            describe(&theme, "space-x-4").unwrap().targets,
            [ClassTarget::Prop("spaceX")]
        );
        let primary = describe(&theme, "bg-primary").unwrap();
        assert!(primary.reads_theme);
        assert_eq!(primary.targets, [ClassTarget::Style(K::BackgroundColor)]);
        assert!(!describe(&theme, "bg-red-500").unwrap().reads_theme);
    }

    /// The tokens a class reads, by name: by its utility, by a variable,
    /// through a role; none for a class of no token.
    #[test]
    fn the_tokens_a_class_reads_by_name() {
        use crate::element::Tokens;
        let mut tokens = Tokens::default();
        tokens.set_color("primary", crate::color::rgb(0x111111));
        tokens.set_color("surface", crate::color::rgb(0xffffff));
        tokens.set_number("radius-card", 12.0);
        tokens.set_shadow("shadow-raised", [crate::BoxShadow::default()]);
        let tokens = tokens.with_roles([("popover", "surface")]);
        let mut theme = Theme::default();
        theme.set_tokens(&tokens);
        let read = |class: &str| describe(&theme, class).unwrap().tokens;
        assert_eq!(read("bg-primary"), ["primary"]);
        assert_eq!(read("md:hover:rounded-card"), ["radius-card"]);
        assert_eq!(read("border-x-primary/50"), ["primary"]);
        assert_eq!(read("shadow-raised"), ["shadow-raised"]);
        assert_eq!(read("bg-(--popover)"), ["surface"]);
        assert!(read("p-4").is_empty());
        assert!(read("bg-red-500").is_empty());
    }

    #[test]
    fn rejected_tokens_have_no_description() {
        let theme = Theme::default();
        for token in ["nope-xyz", "p-", "zz:p-4", "bg-[notacolor]x"] {
            assert_eq!(describe(&theme, token), None, "{token}");
        }
        // Accepted, and nothing to set.
        assert!(describe(&theme, "group").unwrap().targets.is_empty());
        assert!(describe(&theme, "antialiased").unwrap().targets.is_empty());
    }

    /// The utilities table (`utilities.rs`: its keywords and its prefixes)
    /// walked: every utility the parser accepts is described with what it
    /// sets.
    #[test]
    fn every_utility_the_parser_accepts_is_described() {
        let theme = themed();
        let source = include_str!("utilities.rs");
        let names_between = |start: &str, end: &str| -> Vec<String> {
            let from = source.find(start).expect("the table");
            let rest = &source[from..];
            let rest = &rest[..rest[1..].find(end).map_or(rest.len(), |i| i + 1)];
            let mut names = Vec::new();
            let mut it = rest.split('"');
            it.next();
            while let (Some(name), Some(_)) = (it.next(), it.next()) {
                if !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                {
                    names.push(name.to_owned());
                }
            }
            names
        };
        let keywords = names_between("fn keyword(u: &str)", "\nfn ");
        let prefixes = names_between("static PREFIXES", "\n];");
        assert!(
            keywords.len() > 50 && prefixes.len() > 50,
            "{} {}",
            keywords.len(),
            prefixes.len()
        );
        let mut missing = Vec::new();
        let mut accepted = 0;
        let mut check = |utility: String| {
            if let Some(decls) = super::super::utilities::parse(&theme, &utility) {
                accepted += 1;
                // Described, with targets exactly when it declares something
                // (`transform`, `transform-gpu`: accepted, nothing to set).
                match describe(&theme, &utility) {
                    Some(info) if info.targets.is_empty() == decls.is_empty() => {}
                    other => missing.push(format!("{utility}: {decls:?} -> {other:?}")),
                }
            }
        };
        for keyword in keywords {
            check(keyword);
        }
        let samples = [
            "4",
            "2",
            "1",
            "0",
            "full",
            "auto",
            "none",
            "sm",
            "md",
            "red-500",
            "black",
            "[4px]",
            "[#ff0000]",
            "50",
            "x",
            "y",
            "1/2",
            "px",
        ];
        for prefix in prefixes {
            for sample in samples {
                check(format!("{prefix}-{sample}"));
            }
        }
        assert!(accepted > 200, "only {accepted} utilities accepted");
        assert!(
            missing.is_empty(),
            "accepted without a description: {missing:#?}"
        );
    }
}
