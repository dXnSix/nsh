//! The app's location ([`crate::element::location`], TS `location()`):
//! where the user is in the app, as a URL from the app's base
//! (`/sports/football?tab=live#top`). One value per surface, the same on
//! every platform: the web shows it in the address bar and keeps it in the
//! browser's history (`nsh.js`), an embedding app sets it and hears the
//! app's changes of it (its `HostChannel`), and native deep links can feed
//! it the same way.
//!
//! The flow: [`crate::element::navigate`] (TS `navigate`) resolves a URL
//! against the location ([`Location::join`]), sets it at once and sends
//! [`crate::Command::Navigate`] on the root; the core hands each
//! [`Navigation`] to the engine ([`crate::Core::take_navigations`]), which
//! tells the embedding app, and the web's player writes it into the
//! browser's history. A move the platform makes (the browser's back and
//! forward, the embedding app) comes back as [`crate::EventKind::Location`]
//! or [`crate::Core::set_location`].

use std::{fmt, sync::Arc};

/// Where the user is in the app: a path from the app's base with its query
/// and fragment (`/sports/football?tab=live#top`), normalized as URLs are
/// (percent-encoding, `.` and `..` resolved). Cheap to clone.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Location {
    url: Arc<str>,
}

impl Location {
    /// The app's start, `/`.
    pub fn root() -> Location {
        Location { url: "/".into() }
    }

    /// The location `url` names: a path from the app's base (`/sports`),
    /// with a query and a fragment if it has them. Refuses what is not
    /// such a path ([`InvalidLocation`]).
    pub fn parse(url: &str) -> Result<Location, InvalidLocation> {
        if !url.starts_with('/') || url.starts_with("//") {
            return Err(InvalidLocation::new(format!(
                "a location is a path from the app's base, as \"/sports\" ({url:?})"
            )));
        }
        Location::root().join(url)
    }

    /// `url` resolved against this location, as a link on a page at it
    /// would be: `/teams` from the base, `football` next to the last
    /// segment, `?tab=live` and `#top` on this path, `..` up. Refuses URLs
    /// of other sites (a scheme or a host: `https://…`, `//…`, which
    /// [`crate::element::open_url`] opens), and ones over
    /// [`crate::open_url::MAX_URL_LEN`] bytes or with control characters.
    pub fn join(&self, url: &str) -> Result<Location, InvalidLocation> {
        if url.len() > crate::open_url::MAX_URL_LEN {
            return Err(InvalidLocation::new(format!(
                "a location has at most {} bytes (this one has {})",
                crate::open_url::MAX_URL_LEN,
                url.len()
            )));
        }
        if url.chars().any(char::is_control) {
            return Err(InvalidLocation::new(
                "a location has no control characters".into(),
            ));
        }
        if url.starts_with("//") || url.starts_with("\\\\") || url::Url::parse(url).is_ok() {
            return Err(InvalidLocation::new(format!(
                "{url:?} is another site's URL: a location is a path in the app (open other sites with openUrl)"
            )));
        }
        let base = url::Url::parse(&format!("{BASE}{}", self.url))
            .map_err(|e| InvalidLocation::new(format!("{e} ({})", self.url)))?;
        let joined = base
            .join(url)
            .map_err(|e| InvalidLocation::new(format!("{e} ({url:?})")))?;
        if joined.origin() != base.origin() {
            return Err(InvalidLocation::new(format!(
                "{url:?} is another site's URL: a location is a path in the app"
            )));
        }
        let mut location = joined.path().to_owned();
        if let Some(query) = joined.query() {
            location.push('?');
            location.push_str(query);
        }
        if let Some(fragment) = joined.fragment() {
            location.push('#');
            location.push_str(fragment);
        }
        Ok(Location {
            url: location.into(),
        })
    }

    /// The whole location, `/sports/football?tab=live#top`.
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// Its path, `/sports/football`.
    pub fn path(&self) -> &str {
        let end = self.url.find(['?', '#']).unwrap_or(self.url.len());
        &self.url[..end]
    }

    /// Its query without the `?`, `tab=live`, if it has one.
    pub fn query(&self) -> Option<&str> {
        let start = self.url.find('?')?;
        let end = self.url.find('#').unwrap_or(self.url.len());
        (start < end).then(|| &self.url[start + 1..end])
    }

    /// Its fragment without the `#`, `top`, if it has one.
    pub fn fragment(&self) -> Option<&str> {
        self.url.find('#').map(|start| &self.url[start + 1..])
    }
}

/// The base locations are resolved against (no site has it).
const BASE: &str = "http://nsh.invalid";

impl Default for Location {
    fn default() -> Self {
        Location::root()
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.url)
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.url, f)
    }
}

/// Why a URL is no location ([`Location::parse`], [`Location::join`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidLocation {
    message: String,
}

impl InvalidLocation {
    fn new(message: String) -> Self {
        InvalidLocation { message }
    }

    /// What is wrong with the URL.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for InvalidLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InvalidLocation {}

/// What a navigation does to the history (the DOM Navigation API's
/// `history`): on the web, a new browser history entry or the current one
/// changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HistoryBehavior {
    /// A new entry: going back returns to the location before.
    #[default]
    Push,
    /// The current entry: the location before is not gone back to.
    Replace,
}

/// The options of [`crate::element::navigate`]. A [`HistoryBehavior`]
/// converts into them: `navigate("/x", HistoryBehavior::Replace)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct NavigateOptions {
    /// What the navigation does to the history.
    pub history: HistoryBehavior,
}

impl From<HistoryBehavior> for NavigateOptions {
    fn from(history: HistoryBehavior) -> Self {
        NavigateOptions { history }
    }
}

/// A move the app made to another location
/// ([`crate::Command::Navigate`], [`crate::Core::take_navigations`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Navigation {
    /// Where the app went.
    pub location: Location,
    /// What it does to the history.
    pub history: HistoryBehavior,
}

impl Navigation {
    /// The navigation to `location`, doing `history`.
    pub fn new(location: Location, history: HistoryBehavior) -> Self {
        Navigation { location, history }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(url: &str) -> Location {
        Location::parse(url).unwrap()
    }

    #[test]
    fn locations_are_paths_from_the_base() {
        let l = at("/sports/football?tab=live#top");
        assert_eq!(l.as_str(), "/sports/football?tab=live#top");
        assert_eq!(l.path(), "/sports/football");
        assert_eq!(l.query(), Some("tab=live"));
        assert_eq!(l.fragment(), Some("top"));
        assert_eq!(at("/").query(), None);
        assert_eq!(at("/a#b?c").query(), None);
        assert_eq!(at("/a#b?c").fragment(), Some("b?c"));
        assert_eq!(Location::default(), Location::root());
        // Normalized as URLs are.
        assert_eq!(at("/a b/./c/../d").as_str(), "/a%20b/d");
        assert_eq!(at("/../x").as_str(), "/x");
        for url in [
            "",
            "sports",
            "//example.com/x",
            "https://example.com/",
            "/a\nb",
        ] {
            assert!(Location::parse(url).is_err(), "{url:?}");
        }
    }

    #[test]
    fn urls_resolve_against_the_location() {
        let l = at("/sports/football?tab=live#top");
        assert_eq!(l.join("/teams").unwrap().as_str(), "/teams");
        assert_eq!(l.join("tennis").unwrap().as_str(), "/sports/tennis");
        assert_eq!(l.join("../news").unwrap().as_str(), "/news");
        assert_eq!(
            l.join("?tab=table").unwrap().as_str(),
            "/sports/football?tab=table"
        );
        assert_eq!(
            l.join("#scores").unwrap().as_str(),
            "/sports/football?tab=live#scores"
        );
        assert_eq!(l.join("").unwrap().as_str(), "/sports/football?tab=live");
        // Other sites are openUrl's.
        for url in [
            "https://example.com/",
            "mailto:a@example.com",
            "//example.com/x",
            "\\\\example.com/x",
            "/\\example.com/x",
        ] {
            assert!(l.join(url).is_err(), "{url:?}");
        }
        let long = format!("/{}", "a".repeat(crate::open_url::MAX_URL_LEN));
        assert!(l.join(&long).is_err());
    }

    #[test]
    fn a_history_behavior_is_navigate_options() {
        let options: NavigateOptions = HistoryBehavior::Replace.into();
        assert_eq!(options.history, HistoryBehavior::Replace);
        assert_eq!(NavigateOptions::default().history, HistoryBehavior::Push);
    }
}
