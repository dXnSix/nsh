//! The UI fonts of Samsung's Tizen TVs, in place of the Linux desktops':
//! Samsung's TV UI family, then Tizen's (BreezeSans since Tizen 7,
//! TizenSans before). Each comes as a family per script besides the Latin
//! one (`BreezeSansKorean`, `BreezeSansArabic`, ...), whose exact set
//! differs between TVs, so the fallbacks are what is installed of them,
//! not a fixed list. Names match ignoring case and spaces, as TV hosts
//! that load only these families match them.

/// The TV's UI families, in order of preference.
pub(super) const FAMILIES: &[&str] = &["SamsungOneUI", "BreezeSans", "TizenSans"];

/// `name` lowercase, without spaces.
fn normalized(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Where `family` stands among the TV's: the index of the first of
/// [`FAMILIES`] it is (`Ok`) or a script family of (`Err`, as
/// `BreezeSansKorean` of `BreezeSans`); `None` for another family.
fn rank(family: &str) -> Option<Result<usize, usize>> {
    let name = normalized(family);
    FAMILIES.iter().enumerate().find_map(|(i, f)| {
        let f = normalized(f);
        if name == f {
            Some(Ok(i))
        } else {
            name.starts_with(&f).then_some(Err(i))
        }
    })
}

/// The installed ones of [`FAMILIES`] themselves (their script families
/// left out), in its order: the candidates for the UI font.
pub(super) fn ui_candidates<'a>(
    installed: impl IntoIterator<Item = &'a String>,
) -> Vec<&'a String> {
    let mut found: Vec<(usize, &String)> = installed
        .into_iter()
        .filter_map(|f| match rank(f) {
            Some(Ok(i)) => Some((i, f)),
            _ => None,
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, f)| f).collect()
}

/// The fallbacks of UI font `ui` among the `installed` families: every one
/// of the TV's families and their script families but `ui`, in the order
/// of [`FAMILIES`], each family before its script families (those by
/// name).
pub(super) fn fallbacks<'a>(
    installed: impl IntoIterator<Item = &'a String>,
    ui: &str,
) -> Vec<String> {
    let ui = normalized(ui);
    let mut found: Vec<((usize, bool), &String)> = installed
        .into_iter()
        .filter(|f| normalized(f) != ui)
        .filter_map(|f| {
            let key = match rank(f)? {
                Ok(i) => (i, false),
                Err(i) => (i, true),
            };
            Some((key, f))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, f)| f.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_ui_font_is_the_first_tv_family_installed() {
        let installed = names(&["DejaVu Sans", "BreezeSansKorean", "BreezeSans", "TizenSans"]);
        assert_eq!(ui_candidates(&installed), [&installed[2], &installed[3]]);
        // Samsung's family first, whatever the spelling.
        let installed = names(&["BreezeSans", "Samsung One UI"]);
        assert_eq!(ui_candidates(&installed), [&installed[1], &installed[0]]);
        // A script family alone is no UI font.
        assert!(ui_candidates(&names(&["BreezeSansThai"])).is_empty());
        assert!(ui_candidates(&names(&["Noto Sans"])).is_empty());
    }

    #[test]
    fn fallbacks_are_the_tv_families_and_their_scripts() {
        let installed = names(&[
            "TizenSans",
            "BreezeSansThai",
            "Noto Sans",
            "BreezeSansArabic",
            "SamsungOneUIKorean",
            "SamsungOneUI",
            "BreezeSans",
        ]);
        assert_eq!(
            fallbacks(&installed, "SamsungOneUI"),
            names(&[
                "SamsungOneUIKorean",
                "BreezeSans",
                "BreezeSansArabic",
                "BreezeSansThai",
                "TizenSans",
            ])
        );
        // Another UI font: the TV's families all fall back.
        assert_eq!(
            fallbacks(&names(&["BreezeSans", "BreezeSansKorean"]), "Noto Sans"),
            names(&["BreezeSans", "BreezeSansKorean"])
        );
        assert!(fallbacks(&names(&["DejaVu Sans"]), "DejaVu Sans").is_empty());
    }
}
