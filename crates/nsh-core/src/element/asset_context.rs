//! Neutral build-context asset filtering, `when` predicate evaluation, glob
//! matching, alias resolution, and pruned `assets.toml` manifest emission.
//!
//! Packages may ship an `assets/assets.toml` describing rules for their files:
//!
//! ```toml
//! files = ["fonts/**/*.txt"]
//!
//! [[asset]]
//! path = "icons/**/*.svg"
//! exclude = ["icons/draft/**"]
//!
//! [[asset]]
//! path = "brand/acme/logo.svg"
//! alias = "brand/logo.svg"
//! when = { brand = "acme", platform = ["ios", "macos"] }
//! ```
//!
//! Unknown metadata keys on top-level sections and `[[asset]]` entries are
//! ignored during filtering and preserved verbatim in the pruned manifest so
//! downstream catalog consumers see intact metadata for every shipped asset.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::platform::Os;

/// Built-in context dimensions populated by [`BuildContext::from_target`].
pub const BUILTIN_CONTEXT_DIMENSIONS: [&str; 4] = ["platform", "formFactor", "env", "locale"];

/// Active build-time context dimensions used to filter and alias assets.
///
/// Dimensions hold one or more values (for example single-valued
/// `platform = ["ios"]`, `brand = ["acme"]` or multi-valued
/// `locale = ["en", "de"]`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildContext {
    /// Active dimension values keyed by dimension name.
    pub values: BTreeMap<String, Vec<String>>,
}

impl BuildContext {
    /// An empty context with no dimensions set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a context from target metadata, release mode, active locales, and
    /// explicit dimension overrides.
    ///
    /// Automatically populates:
    /// - `platform`: `ios`, `android`, `macos`, `windows`, `linux`, `web`, or `tizen`
    /// - `formFactor`: `mobile`, `desktop`, `web`, or `tv`
    /// - `env`: `production` when `release`, else `development`
    /// - `locale`: `locales` when non-empty
    ///
    /// Any key present in `overrides` replaces the auto-populated value for
    /// that dimension.
    pub fn from_target(
        os: Option<Os>,
        release: bool,
        locales: &[String],
        overrides: &BTreeMap<String, Vec<String>>,
    ) -> Self {
        let mut values = BTreeMap::new();
        if let Some(os) = os {
            values.insert("platform".to_owned(), vec![os.name().to_owned()]);
            values.insert(
                "formFactor".to_owned(),
                vec![form_factor_for(os).to_owned()],
            );
        }
        values.insert(
            "env".to_owned(),
            vec![if release { "production" } else { "development" }.to_owned()],
        );
        let dedup_locales = dedup_non_empty(locales.iter().map(String::as_str));
        if !dedup_locales.is_empty() {
            values.insert("locale".to_owned(), dedup_locales);
        }
        for (key, vals) in overrides {
            let clean = dedup_non_empty(vals.iter().map(String::as_str));
            if !clean.is_empty() {
                values.insert(key.clone(), clean);
            }
        }
        Self { values }
    }

    /// Returns a copy with `key` set to a single `value`.
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let value = value.into();
        if !value.is_empty() {
            self.values.insert(key.into(), vec![value]);
        }
        self
    }

    /// Returns a copy with `key` set to `values`.
    pub fn with_values(
        mut self,
        key: impl Into<String>,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let vals: Vec<String> = values.into_iter().map(Into::into).collect();
        let clean = dedup_non_empty(vals.iter().map(String::as_str));
        if !clean.is_empty() {
            self.values.insert(key.into(), clean);
        }
        self
    }

    /// Sets `key` to `values` (deduplicated, non-empty).
    pub fn set(&mut self, key: impl Into<String>, values: Vec<String>) {
        let clean = dedup_non_empty(values.iter().map(String::as_str));
        if clean.is_empty() {
            self.values.remove(&key.into());
        } else {
            self.values.insert(key.into(), clean);
        }
    }

    /// Active values for dimension `key`, if set.
    pub fn get(&self, key: &str) -> Option<&[String]> {
        self.values.get(key).map(Vec::as_slice)
    }

    /// Whether no dimensions are set.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The target [`Os`] named by the `platform` dimension, if any.
    pub fn target_os(&self) -> Option<Os> {
        let name = self.values.get("platform")?.first()?.as_str();
        parse_os_name(name)
    }

    /// Compile-time bundler `define` entries for every active dimension:
    /// `process.env.NSH_CONTEXT_<UPPER>` and `import.meta.env.NSH_CONTEXT_<UPPER>`.
    ///
    /// Single-valued dimensions become JSON string literals (e.g. `"\"acme\""`);
    /// multi-valued dimensions are comma-joined (e.g. `"\"en,de\""`).
    /// For camelCase keys such as `formFactor`, both `NSH_CONTEXT_FORM_FACTOR`
    /// and `NSH_CONTEXT_FORMFACTOR` are emitted so either spelling folds cleanly.
    pub fn define_constants(&self) -> BTreeMap<String, String> {
        let mut defines = BTreeMap::new();
        for (key, vals) in &self.values {
            if vals.is_empty() {
                continue;
            }
            let joined = vals.join(",");
            let literal = serde_json::Value::String(joined).to_string();
            let upper = env_upper_key(key);
            let snake = env_snake_upper_key(key);
            for name in [upper, snake] {
                if name.is_empty() {
                    continue;
                }
                defines.insert(format!("process.env.NSH_CONTEXT_{name}"), literal.clone());
                defines.insert(
                    format!("import.meta.env.NSH_CONTEXT_{name}"),
                    literal.clone(),
                );
            }
        }
        defines
    }
}

impl fmt::Display for BuildContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        for (i, (key, vals)) in self.values.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            if vals.len() == 1 {
                write!(f, "{key} = {:?}", vals[0])?;
            } else {
                write!(f, "{key} = {vals:?}")?;
            }
        }
        f.write_str("}")
    }
}

/// Parses an OS name (`ios`, `android`, `macos`, `windows`, `linux`, `web`, `tizen`).
pub fn parse_os_name(name: &str) -> Option<Os> {
    match name {
        "ios" => Some(Os::Ios),
        "android" => Some(Os::Android),
        "macos" => Some(Os::MacOs),
        "windows" => Some(Os::Windows),
        "linux" => Some(Os::Linux),
        "web" => Some(Os::Web),
        "tizen" => Some(Os::Tizen),
        _ => None,
    }
}

fn form_factor_for(os: Os) -> &'static str {
    match os {
        Os::Ios | Os::Android => "mobile",
        Os::MacOs | Os::Windows | Os::Linux => "desktop",
        Os::Web => "web",
        Os::Tizen => "tv",
    }
}

fn env_upper_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn env_snake_upper_key(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    let mut prev_lower_or_digit = false;
    for c in key.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && prev_lower_or_digit {
                out.push('_');
            }
            out.push(c.to_ascii_uppercase());
            prev_lower_or_digit = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else {
            if !out.ends_with('_') {
                out.push('_');
            }
            prev_lower_or_digit = false;
        }
    }
    out
}

fn dedup_non_empty<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        let trimmed = item.trim();
        if !trimmed.is_empty() && !out.iter().any(|existing| existing == trimmed) {
            out.push(trimmed.to_owned());
        }
    }
    out
}

/// Build-time inclusion predicate attached to an `[[asset]]` entry.
///
/// Semantics:
/// - **AND** across keys (every key in [`Self::conditions`] must pass).
/// - **OR** across positive values within a key (at least one positive value
///   must be present in the active context for that key).
/// - **`!value` negation** (fails if the active context contains `value`).
/// - An empty predicate always matches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WhenPredicate {
    /// Required/negated values keyed by dimension name.
    pub conditions: BTreeMap<String, Vec<String>>,
}

impl WhenPredicate {
    /// An empty predicate (always matches).
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this predicate has no conditions.
    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }

    /// Evaluates this predicate against `ctx`.
    pub fn matches(&self, ctx: &BuildContext) -> bool {
        for (key, expected) in &self.conditions {
            if expected.is_empty() {
                continue;
            }
            let active: &[String] = ctx.values.get(key).map(Vec::as_slice).unwrap_or(&[]);
            let mut has_positive = false;
            let mut positive_matched = false;
            for item in expected {
                if let Some(negated) = item.strip_prefix('!') {
                    if active.iter().any(|v| v == negated) {
                        return false;
                    }
                } else {
                    has_positive = true;
                    if active.iter().any(|v| v == item) {
                        positive_matched = true;
                    }
                }
            }
            if has_positive && !positive_matched {
                return false;
            }
        }
        true
    }
}

impl fmt::Display for WhenPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{ ")?;
        for (i, (key, vals)) in self.conditions.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            if vals.len() == 1 {
                write!(f, "{key} = {:?}", vals[0])?;
            } else {
                write!(f, "{key} = {vals:?}")?;
            }
        }
        f.write_str(" }")
    }
}

/// Normalizes an asset relative path to `/`-separated form without leading `./` or `/`.
pub fn normalize_rel_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    while let Some(rest) = normalized.strip_prefix("./") {
        normalized = rest.to_owned();
    }
    while let Some(rest) = normalized.strip_prefix('/') {
        normalized = rest.to_owned();
    }
    normalized
}

/// Whether `pattern` contains glob syntax (`*`, `?`, `{`).
pub fn is_glob(pattern: &str) -> bool {
    pattern.contains(['*', '?', '{'])
}

/// Whether relative asset `path` matches `pattern` (`*`, `**`, `?`, `{a,b}`).
pub fn glob_matches(pattern: &str, path: &str) -> bool {
    let path = normalize_rel_path(path);
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let pattern = normalize_rel_path(pattern);
    for expanded in expand_braces(&pattern) {
        let segments: Vec<&str> = expanded.split('/').filter(|s| !s.is_empty()).collect();
        if path_matches(&segments, &parts) {
            return true;
        }
    }
    false
}

fn path_matches(segments: &[&str], parts: &[&str]) -> bool {
    match (segments.first(), parts.first()) {
        (None, None) => true,
        (Some(&"**"), _) => {
            path_matches(&segments[1..], parts)
                || (!parts.is_empty() && path_matches(segments, &parts[1..]))
        }
        (Some(s), Some(p)) => segment_matches(s, p) && path_matches(&segments[1..], &parts[1..]),
        _ => false,
    }
}

fn segment_matches(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    let (mut i, mut j) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}

fn expand_braces(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_owned()];
    };
    let mut depth = 0usize;
    let mut close = None;
    for (i, c) in pattern[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(close) = close else {
        return vec![pattern.to_owned()];
    };
    let (head, body, tail) = (
        &pattern[..open],
        &pattern[open + 1..close],
        &pattern[close + 1..],
    );
    let mut options = Vec::new();
    let (mut depth, mut from) = (0usize, 0usize);
    for (i, c) in body.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                options.push(&body[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    options.push(&body[from..]);
    options
        .into_iter()
        .flat_map(|o| expand_braces(&format!("{head}{o}{tail}")))
        .collect()
}

/// Errors returned while parsing or evaluating an `assets.toml` filter manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetContextError {
    /// Failed to parse `assets.toml`.
    Parse(String),
    /// Two or more active entries or files resolve to the same alias.
    AliasConflict {
        /// Package name.
        package: String,
        /// The conflicting alias path.
        alias: String,
        /// Physical files or manifest entries competing for `alias`.
        sources: Vec<String>,
    },
    /// Invalid alias path (empty or containing parent traversal).
    InvalidAlias {
        /// Package name.
        package: String,
        /// The invalid alias string.
        alias: String,
    },
}

impl fmt::Display for AssetContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(msg) => write!(f, "invalid assets.toml: {msg}"),
            Self::AliasConflict {
                package,
                alias,
                sources,
            } => write!(
                f,
                "package `{package}`: alias `{alias}` is claimed by multiple active assets: {}",
                sources.join(", ")
            ),
            Self::InvalidAlias { package, alias } => {
                write!(f, "package `{package}`: invalid asset alias `{alias}`")
            }
        }
    }
}

impl std::error::Error for AssetContextError {}

/// One `[[asset]]` rule parsed from `assets.toml` for build-time filtering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetFilterEntry {
    /// Exact relative path or glob pattern (`*`, `**`, `?`, `{a,b}`).
    pub path: String,
    /// Optional glob patterns excluded from this rule.
    pub exclude: Vec<String>,
    /// Build-time inclusion predicate (`when`).
    pub when: WhenPredicate,
    /// Optional logical path override (`alias`).
    pub alias: Option<String>,
    /// Original TOML text of this `[[asset]]` block (preserving unknown keys).
    raw_block: String,
}

impl AssetFilterEntry {
    /// Whether `rel_path` matches [`Self::path`] (ignoring [`Self::exclude`]).
    pub fn matches_path(&self, rel_path: &str) -> bool {
        glob_matches(&self.path, rel_path)
    }

    /// The first [`Self::exclude`] pattern that excludes `rel_path`, if any.
    pub fn excluded_by(&self, rel_path: &str) -> Option<&str> {
        self.exclude
            .iter()
            .map(String::as_str)
            .find(|ex| glob_matches(ex, rel_path))
    }

    /// Specificity rank for tie-breaking when multiple active rules match one file:
    /// exact path > specific glob > broad glob (`**`).
    fn specificity(&self) -> (u8, usize, usize) {
        let norm = normalize_rel_path(&self.path);
        if !is_glob(&norm) {
            return (3, usize::MAX, norm.len());
        }
        let segments: Vec<&str> = norm.split('/').filter(|s| !s.is_empty()).collect();
        let has_double_star = segments.contains(&"**");
        let literal_segments = segments.iter().filter(|s| !is_glob(s)).count();
        let tier = if has_double_star { 1 } else { 2 };
        (tier, literal_segments, norm.len())
    }
}

/// Neutral `assets.toml` build-filter manifest.
///
/// Parses `files` and `[[asset]]` (`path`, `exclude`, `when`, `alias`) while
/// ignoring and preserving unknown metadata keys for pruned manifest emission.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetFilterManifest {
    /// Top-level `files = [...]` shorthand patterns.
    pub files: Vec<String>,
    /// Parsed `[[asset]]` filter entries in declaration order.
    pub entries: Vec<AssetFilterEntry>,
    /// Top-level TOML blocks outside `[[asset]]`, preserved in pruned output.
    top_level_chunks: Vec<String>,
}

impl AssetFilterManifest {
    /// Parses a TOML `assets.toml` manifest without rejecting unknown fields.
    pub fn parse(toml: &str) -> Result<Self, AssetContextError> {
        parse_manifest(toml)
    }

    /// Emits a pruned `assets.toml` containing top-level sections and only
    /// `[[asset]]` entries whose `when` predicate matches `ctx`, preserving all
    /// unknown metadata keys on surviving entries.
    pub fn pruned_toml(&self, ctx: &BuildContext) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for chunk in &self.top_level_chunks {
            let trimmed = chunk.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed);
            }
        }
        for entry in &self.entries {
            if entry.when.matches(ctx) {
                let trimmed = entry.raw_block.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed);
                }
            }
        }
        if parts.is_empty() {
            String::new()
        } else {
            let mut out = parts.join("\n\n");
            out.push('\n');
            out
        }
    }
}

/// One physical file resolved for a package under a [`BuildContext`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAsset {
    /// Package-relative logical path shipped in the app (the alias if aliased,
    /// otherwise the normalized physical path).
    pub logical_path: String,
    /// Package-relative physical path under `<package>/assets/`.
    pub physical_path: String,
    /// Applied alias when [`Self::logical_path`] differs from [`Self::physical_path`].
    pub alias: Option<String>,
}

/// Why an asset path was excluded under a [`BuildContext`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExclusionReason {
    /// Excluded because none of the matching `[[asset]]` entries' `when`
    /// predicates matched the active context.
    When(Vec<WhenPredicate>),
    /// Excluded by an `exclude` glob pattern in `assets.toml`.
    ExcludedByPattern(String),
}

impl fmt::Display for ExclusionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::When(predicates) if predicates.len() == 1 => {
                write!(f, "requires when = {}", predicates[0])
            }
            Self::When(predicates) => {
                let list: Vec<String> = predicates.iter().map(ToString::to_string).collect();
                write!(f, "requires one of when = {}", list.join(" or "))
            }
            Self::ExcludedByPattern(pattern) => {
                write!(f, "excluded by `exclude = [{pattern:?}]` in assets.toml")
            }
        }
    }
}

/// Result of resolving a package's assets against its manifest and a [`BuildContext`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAssets {
    /// Package name (without leading `@`).
    pub package: String,
    /// Shipped assets sorted by [`ResolvedAsset::logical_path`].
    pub included: Vec<ResolvedAsset>,
    /// Active alias mappings (`logical_path -> physical_path`).
    pub aliases: BTreeMap<String, String>,
    /// Excluded physical and aliased paths mapped to their exclusion reasons.
    pub excluded: BTreeMap<String, ExclusionReason>,
    /// Pruned `assets.toml` containing only surviving entries with intact metadata.
    pub pruned_manifest: String,
}

/// Evaluates `files` (package-relative paths under `assets/`) against `manifest`
/// and `ctx`, returning included assets, resolved aliases, excluded paths with
/// reasons, and the pruned `assets.toml` text.
pub fn resolve_package_assets(
    package: &str,
    files: &[String],
    manifest: &AssetFilterManifest,
    ctx: &BuildContext,
) -> Result<ResolvedAssets, AssetContextError> {
    // Validate active alias declarations first, catching direct alias conflicts
    // between surviving `[[asset]]` entries even before file matching.
    let mut active_alias_entries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut inactive_alias_predicates: BTreeMap<String, Vec<WhenPredicate>> = BTreeMap::new();

    for entry in &manifest.entries {
        if let Some(raw_alias) = &entry.alias {
            let alias = validate_alias(package, raw_alias)?;
            if entry.when.matches(ctx) {
                active_alias_entries
                    .entry(alias)
                    .or_default()
                    .push(entry.path.clone());
            } else {
                inactive_alias_predicates
                    .entry(alias)
                    .or_default()
                    .push(entry.when.clone());
            }
        }
    }
    for (alias, sources) in &active_alias_entries {
        if sources.len() > 1 {
            return Err(AssetContextError::AliasConflict {
                package: package.to_owned(),
                alias: alias.clone(),
                sources: sources.clone(),
            });
        }
    }

    let mut normalized_files: Vec<String> = files
        .iter()
        .map(|f| normalize_rel_path(f))
        .filter(|f| !f.is_empty() && f != "assets.toml")
        .collect();
    normalized_files.sort();
    normalized_files.dedup();

    let mut included_raw: Vec<ResolvedAsset> = Vec::new();
    let mut excluded: BTreeMap<String, ExclusionReason> = BTreeMap::new();
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    let mut alias_claims: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for rel in &normalized_files {
        let in_top_files = manifest.files.iter().any(|pat| glob_matches(pat, rel));

        let mut non_excluded: Vec<(usize, &AssetFilterEntry)> = Vec::new();
        let mut excluded_by: Option<String> = None;

        for (idx, entry) in manifest.entries.iter().enumerate() {
            if entry.matches_path(rel) {
                if let Some(pat) = entry.excluded_by(rel) {
                    if excluded_by.is_none() {
                        excluded_by = Some(pat.to_owned());
                    }
                } else {
                    non_excluded.push((idx, entry));
                }
            }
        }

        if !non_excluded.is_empty() {
            let mut active: Vec<(usize, &AssetFilterEntry)> = non_excluded
                .iter()
                .copied()
                .filter(|(_, entry)| entry.when.matches(ctx))
                .collect();

            if active.is_empty() {
                if in_top_files {
                    included_raw.push(ResolvedAsset {
                        logical_path: rel.clone(),
                        physical_path: rel.clone(),
                        alias: None,
                    });
                } else {
                    let predicates = non_excluded
                        .into_iter()
                        .map(|(_, entry)| entry.when.clone())
                        .collect();
                    excluded.insert(rel.clone(), ExclusionReason::When(predicates));
                }
                continue;
            }

            // Pick the highest-specificity surviving rule (later declaration wins ties).
            active.sort_by_key(|(idx, entry)| (entry.specificity(), *idx));
            let winning_alias = active
                .iter()
                .rev()
                .find_map(|(_, entry)| entry.alias.as_deref());

            let (logical_path, alias) = match winning_alias {
                Some(raw_alias) => {
                    let alias = validate_alias(package, raw_alias)?;
                    alias_claims
                        .entry(alias.clone())
                        .or_default()
                        .push(rel.clone());
                    (alias.clone(), Some(alias))
                }
                None => (rel.clone(), None),
            };
            included_raw.push(ResolvedAsset {
                logical_path,
                physical_path: rel.clone(),
                alias,
            });
        } else if let Some(pat) = excluded_by {
            if in_top_files {
                included_raw.push(ResolvedAsset {
                    logical_path: rel.clone(),
                    physical_path: rel.clone(),
                    alias: None,
                });
            } else {
                excluded.insert(rel.clone(), ExclusionReason::ExcludedByPattern(pat));
            }
        } else {
            // Unmanaged file not matched by any `[[asset]]` rule: ship as-is.
            included_raw.push(ResolvedAsset {
                logical_path: rel.clone(),
                physical_path: rel.clone(),
                alias: None,
            });
        }
    }

    // Check if any alias was claimed by more than one physical file.
    for (alias, sources) in &alias_claims {
        if sources.len() > 1 {
            return Err(AssetContextError::AliasConflict {
                package: package.to_owned(),
                alias: alias.clone(),
                sources: sources.clone(),
            });
        }
    }

    // Check if an aliased logical path collides with a non-aliased included file.
    let mut by_logical: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &included_raw {
        by_logical
            .entry(item.logical_path.clone())
            .or_default()
            .push(item.physical_path.clone());
    }
    for (logical, sources) in &by_logical {
        if sources.len() > 1 {
            return Err(AssetContextError::AliasConflict {
                package: package.to_owned(),
                alias: logical.clone(),
                sources: sources.clone(),
            });
        }
    }

    // Propagate aliases to density siblings (`logo@2x.png`) of aliased raster
    // images when the sibling is included without an explicit alias and does
    // not collide with any other logical path.
    let occupied: BTreeSet<String> = included_raw
        .iter()
        .map(|item| item.logical_path.clone())
        .collect();
    let aliased_bases: Vec<(String, String)> = included_raw
        .iter()
        .filter_map(|item| {
            item.alias
                .as_ref()
                .map(|alias| (item.physical_path.clone(), alias.clone()))
        })
        .collect();

    let mut auto_sibling_aliases: BTreeMap<String, String> = BTreeMap::new();
    for (phys_base, alias_base) in &aliased_bases {
        for scale in crate::IMAGE_SCALES {
            if scale == 1.0 {
                continue;
            }
            let (Some(phys_var), Some(alias_var)) = (
                density_variant_path(phys_base, scale),
                density_variant_path(alias_base, scale),
            ) else {
                continue;
            };
            if !occupied.contains(&alias_var) {
                auto_sibling_aliases.insert(phys_var, alias_var);
            }
        }
    }

    for item in &mut included_raw {
        if item.alias.is_none()
            && let Some(alias_var) = auto_sibling_aliases.get(&item.physical_path)
        {
            item.logical_path = alias_var.clone();
            item.alias = Some(alias_var.clone());
        }
        if let Some(alias) = &item.alias {
            aliases.insert(alias.clone(), item.physical_path.clone());
        }
    }

    // Record excluded alias targets (aliases whose only matching entries were
    // inactive in this context, so importing `asset:/@pkg/<alias>` fails with
    // an actionable `when` explanation instead of "closest match").
    for (alias, predicates) in inactive_alias_predicates {
        if !aliases.contains_key(&alias) && !occupied.contains(&alias) {
            excluded.insert(alias, ExclusionReason::When(predicates));
        }
    }

    included_raw.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
    let pruned_manifest = manifest.pruned_toml(ctx);

    Ok(ResolvedAssets {
        package: package.to_owned(),
        included: included_raw,
        aliases,
        excluded,
        pruned_manifest,
    })
}

fn validate_alias(package: &str, raw: &str) -> Result<String, AssetContextError> {
    let normalized = normalize_rel_path(raw.trim());
    if normalized.is_empty()
        || normalized
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return Err(AssetContextError::InvalidAlias {
            package: package.to_owned(),
            alias: raw.to_owned(),
        });
    }
    Ok(normalized)
}

fn density_variant_path(path: &str, scale: f32) -> Option<String> {
    let (dir, file) = match path.rsplit_once('/') {
        Some((dir, file)) => (Some(dir), file),
        None => (None, path),
    };
    let (stem, ext) = file.rsplit_once('.')?;
    // Do not double-suffix an already scaled name.
    if stem
        .rsplit_once('@')
        .and_then(|(_, d)| d.strip_suffix('x'))
        .is_some_and(|d| d.parse::<f64>().is_ok())
    {
        return None;
    }
    let variant_file = format!("{stem}@{scale}x.{ext}");
    Some(match dir {
        Some(dir) => format!("{dir}/{variant_file}"),
        None => variant_file,
    })
}

// ---------------------------------------------------------------------------
// Minimal, lossless-block TOML parser for neutral `assets.toml` filtering.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum TomlValue {
    String(String),
    Array(Vec<TomlValue>),
    Table(BTreeMap<String, TomlValue>),
    Other,
}

/// Splits `toml` into top-level chunks and `[[asset]]` entry blocks, then
/// parses the filtering keys (`files`, `path`, `exclude`, `when`, `alias`)
/// while ignoring unknown metadata fields and keeping raw blocks intact.
fn parse_manifest(toml: &str) -> Result<AssetFilterManifest, AssetContextError> {
    enum RawSection {
        TopLevel(String),
        Asset(String),
    }

    let mut sections: Vec<RawSection> = Vec::new();
    let mut current_is_asset = false;
    let mut current_buf = String::new();

    // Walk line-by-line tracking multiline string state so `[[asset]]` inside a
    // multiline string is never mistaken for a section header.
    let mut ml_delim: Option<&'static str> = None;
    for line in toml.split_inclusive('\n') {
        let trimmed = line.trim();
        let header = if ml_delim.is_none() {
            parse_header_line(trimmed)
        } else {
            None
        };

        if let Some((is_array, name)) = header {
            if is_array && name == "asset" {
                if !current_buf.is_empty() {
                    if current_is_asset {
                        sections.push(RawSection::Asset(std::mem::take(&mut current_buf)));
                    } else {
                        sections.push(RawSection::TopLevel(std::mem::take(&mut current_buf)));
                    }
                }
                current_is_asset = true;
                current_buf.push_str(line);
            } else if current_is_asset && !is_array && name.starts_with("asset.") {
                // Subtable belonging to the current `[[asset]]` entry (e.g. `[asset.when]`).
                current_buf.push_str(line);
            } else {
                // Any other table header belongs to top-level sections.
                if current_is_asset {
                    sections.push(RawSection::Asset(std::mem::take(&mut current_buf)));
                    current_is_asset = false;
                }
                current_buf.push_str(line);
            }
        } else {
            current_buf.push_str(line);
        }

        update_multiline_state(line, &mut ml_delim);
    }

    if ml_delim.is_some() {
        return Err(AssetContextError::Parse(
            "unterminated multiline string".to_owned(),
        ));
    }

    if !current_buf.is_empty() {
        if current_is_asset {
            sections.push(RawSection::Asset(current_buf));
        } else {
            sections.push(RawSection::TopLevel(current_buf));
        }
    }

    let mut manifest = AssetFilterManifest::default();

    for section in sections {
        match section {
            RawSection::TopLevel(raw) => {
                let table = parse_toml_section(&raw)?;
                if let Some(files_val) = table.get("files") {
                    let list = expect_string_list(files_val, "files")?;
                    manifest.files.extend(list);
                }
                if !raw.trim().is_empty() {
                    manifest.top_level_chunks.push(raw);
                }
            }
            RawSection::Asset(raw) => {
                let entry = parse_asset_block(&raw)?;
                manifest.entries.push(entry);
            }
        }
    }

    Ok(manifest)
}

fn parse_header_line(trimmed: &str) -> Option<(bool, String)> {
    let body = strip_line_comment(trimmed).trim();
    if let Some(rest) = body.strip_prefix("[[")
        && let Some(inner) = rest.strip_suffix("]]")
    {
        return Some((true, normalize_header_name(inner)));
    }
    if let Some(rest) = body.strip_prefix('[')
        && !rest.starts_with('[')
        && let Some(inner) = rest.strip_suffix(']')
    {
        return Some((false, normalize_header_name(inner)));
    }
    None
}

fn normalize_header_name(inner: &str) -> String {
    inner
        .split('.')
        .map(|part| part.trim().trim_matches('"').trim_matches('\''))
        .collect::<Vec<_>>()
        .join(".")
}

fn strip_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(b'"') => {
                if b == b'\\' {
                    i += 2;
                    continue;
                }
                if b == b'"' {
                    quote = None;
                }
            }
            Some(b'\'') => {
                if b == b'\'' {
                    quote = None;
                }
            }
            _ => {
                if b == b'"' || b == b'\'' {
                    quote = Some(b);
                } else if b == b'#' {
                    return &line[..i];
                }
            }
        }
        i += 1;
    }
    line
}

fn update_multiline_state(line: &str, ml_delim: &mut Option<&'static str>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut single_quote: Option<u8> = None;
    while i < bytes.len() {
        if let Some(delim) = *ml_delim {
            let db = delim.as_bytes();
            if delim == "\"\"\"" && bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i..].starts_with(db) {
                *ml_delim = None;
                i += 3;
                continue;
            }
            i += 1;
            continue;
        }
        if let Some(q) = single_quote {
            if q == b'"' && bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == q {
                single_quote = None;
            }
            i += 1;
            continue;
        }
        if bytes[i] == b'#' {
            break;
        }
        if bytes[i..].starts_with(b"\"\"\"") {
            *ml_delim = Some("\"\"\"");
            i += 3;
            continue;
        }
        if bytes[i..].starts_with(b"'''") {
            *ml_delim = Some("'''");
            i += 3;
            continue;
        }
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            single_quote = Some(bytes[i]);
        }
        i += 1;
    }
}

fn parse_asset_block(raw: &str) -> Result<AssetFilterEntry, AssetContextError> {
    let table = parse_toml_section(raw)?;
    let path = match table.get("path") {
        Some(TomlValue::String(s)) => s.clone(),
        Some(_) => {
            return Err(AssetContextError::Parse(
                "[[asset]].path must be a string".to_owned(),
            ));
        }
        None => {
            return Err(AssetContextError::Parse(
                "[[asset]] entry is missing required `path`".to_owned(),
            ));
        }
    };
    let exclude = match table.get("exclude") {
        None => Vec::new(),
        Some(val) => expect_string_list(val, "[[asset]].exclude")?,
    };
    let alias = match table.get("alias") {
        None => None,
        Some(TomlValue::String(s)) => Some(s.clone()),
        Some(_) => {
            return Err(AssetContextError::Parse(
                "[[asset]].alias must be a string".to_owned(),
            ));
        }
    };
    let when = match table.get("when") {
        None => WhenPredicate::default(),
        Some(TomlValue::Table(map)) => {
            let mut conditions = BTreeMap::new();
            for (k, v) in map {
                let vals = match v {
                    TomlValue::String(s) => vec![s.clone()],
                    TomlValue::Array(_) => expect_string_list(v, &format!("[[asset]].when.{k}"))?,
                    _ => {
                        return Err(AssetContextError::Parse(format!(
                            "[[asset]].when.{k} must be a string or array of strings"
                        )));
                    }
                };
                conditions.insert(k.clone(), vals);
            }
            WhenPredicate { conditions }
        }
        Some(_) => {
            return Err(AssetContextError::Parse(
                "[[asset]].when must be an inline table or [asset.when] section".to_owned(),
            ));
        }
    };
    Ok(AssetFilterEntry {
        path,
        exclude,
        when,
        alias,
        raw_block: raw.to_owned(),
    })
}

fn expect_string_list(val: &TomlValue, field: &str) -> Result<Vec<String>, AssetContextError> {
    match val {
        TomlValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    TomlValue::String(s) => out.push(s.clone()),
                    _ => {
                        return Err(AssetContextError::Parse(format!(
                            "{field} must be an array of strings"
                        )));
                    }
                }
            }
            Ok(out)
        }
        _ => Err(AssetContextError::Parse(format!(
            "{field} must be an array of strings"
        ))),
    }
}

struct TomlCursor<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> TomlCursor<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_ws_and_comments(&mut self, allow_newlines: bool) {
        loop {
            while let Some(c) = self.peek() {
                if c == ' ' || c == '\t' || (allow_newlines && (c == '\n' || c == '\r')) {
                    self.bump();
                } else {
                    break;
                }
            }
            if self.peek() == Some('#') {
                while let Some(c) = self.bump() {
                    if c == '\n' {
                        break;
                    }
                }
                if !allow_newlines {
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn parse_key_path(&mut self) -> Result<Vec<String>, AssetContextError> {
        let mut parts = Vec::new();
        loop {
            self.skip_ws_and_comments(false);
            parts.push(self.parse_single_key()?);
            self.skip_ws_and_comments(false);
            if self.peek() == Some('.') {
                self.bump();
            } else {
                break;
            }
        }
        Ok(parts)
    }

    fn parse_single_key(&mut self) -> Result<String, AssetContextError> {
        match self.peek() {
            Some('"') => self.parse_basic_string(),
            Some('\'') => self.parse_literal_string(),
            Some(_) => {
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                        self.bump();
                    } else {
                        break;
                    }
                }
                if self.pos == start {
                    return Err(AssetContextError::Parse(format!(
                        "expected key near {:?}",
                        &self.rest()[..self.rest().len().min(20)]
                    )));
                }
                Ok(self.src[start..self.pos].to_owned())
            }
            None => Err(AssetContextError::Parse(
                "unexpected end of input while reading key".to_owned(),
            )),
        }
    }

    fn parse_value(&mut self) -> Result<TomlValue, AssetContextError> {
        self.skip_ws_and_comments(false);
        if self.rest().starts_with("\"\"\"") {
            return self.parse_multiline_basic_string().map(TomlValue::String);
        }
        if self.rest().starts_with("'''") {
            return self.parse_multiline_literal_string().map(TomlValue::String);
        }
        match self.peek() {
            Some('"') => self.parse_basic_string().map(TomlValue::String),
            Some('\'') => self.parse_literal_string().map(TomlValue::String),
            Some('[') => self.parse_array(),
            Some('{') => self.parse_inline_table(),
            Some(_) => {
                // Number, boolean, datetime, etc. — consume until separator.
                while let Some(c) = self.peek() {
                    if matches!(c, '\n' | '\r' | '#' | ',' | ']' | '}') {
                        break;
                    }
                    self.bump();
                }
                Ok(TomlValue::Other)
            }
            None => Err(AssetContextError::Parse(
                "unexpected end of input while reading value".to_owned(),
            )),
        }
    }

    fn parse_basic_string(&mut self) -> Result<String, AssetContextError> {
        if self.bump() != Some('"') {
            return Err(AssetContextError::Parse("expected '\"'".to_owned()));
        }
        let mut out = String::new();
        while let Some(c) = self.bump() {
            match c {
                '"' => return Ok(out),
                '\\' => out.push(self.parse_escape()?),
                '\n' | '\r' => {
                    return Err(AssetContextError::Parse(
                        "unterminated basic string".to_owned(),
                    ));
                }
                other => out.push(other),
            }
        }
        Err(AssetContextError::Parse(
            "unterminated basic string".to_owned(),
        ))
    }

    fn parse_multiline_basic_string(&mut self) -> Result<String, AssetContextError> {
        self.pos += 3;
        if self.rest().starts_with("\r\n") {
            self.pos += 2;
        } else if self.rest().starts_with('\n') {
            self.pos += 1;
        }
        let mut out = String::new();
        while !self.rest().is_empty() {
            if self.rest().starts_with("\"\"\"") {
                self.pos += 3;
                return Ok(out);
            }
            match self.bump() {
                Some('\\') => {
                    if matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
                        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
                            self.bump();
                        }
                    } else {
                        out.push(self.parse_escape()?);
                    }
                }
                Some(c) => out.push(c),
                None => break,
            }
        }
        Err(AssetContextError::Parse(
            "unterminated multiline basic string".to_owned(),
        ))
    }

    fn parse_literal_string(&mut self) -> Result<String, AssetContextError> {
        if self.bump() != Some('\'') {
            return Err(AssetContextError::Parse("expected '\''".to_owned()));
        }
        let start = self.pos;
        while let Some(c) = self.bump() {
            if c == '\'' {
                return Ok(self.src[start..self.pos - 1].to_owned());
            }
            if c == '\n' || c == '\r' {
                return Err(AssetContextError::Parse(
                    "unterminated literal string".to_owned(),
                ));
            }
        }
        Err(AssetContextError::Parse(
            "unterminated literal string".to_owned(),
        ))
    }

    fn parse_multiline_literal_string(&mut self) -> Result<String, AssetContextError> {
        self.pos += 3;
        if self.rest().starts_with("\r\n") {
            self.pos += 2;
        } else if self.rest().starts_with('\n') {
            self.pos += 1;
        }
        let start = self.pos;
        if let Some(rel) = self.rest().find("'''") {
            let out = self.src[start..self.pos + rel].to_owned();
            self.pos += rel + 3;
            return Ok(out);
        }
        Err(AssetContextError::Parse(
            "unterminated multiline literal string".to_owned(),
        ))
    }

    fn parse_escape(&mut self) -> Result<char, AssetContextError> {
        match self.bump() {
            Some('"') => Ok('"'),
            Some('\\') => Ok('\\'),
            Some('n') => Ok('\n'),
            Some('r') => Ok('\r'),
            Some('t') => Ok('\t'),
            Some('b') => Ok('\u{0008}'),
            Some('f') => Ok('\u{000C}'),
            Some('u') => self.parse_unicode_escape(4),
            Some('U') => self.parse_unicode_escape(8),
            Some(other) => Err(AssetContextError::Parse(format!(
                "invalid escape sequence `\\{other}`"
            ))),
            None => Err(AssetContextError::Parse(
                "unterminated escape sequence".to_owned(),
            )),
        }
    }

    fn parse_unicode_escape(&mut self, digits: usize) -> Result<char, AssetContextError> {
        let hex: String = (0..digits).filter_map(|_| self.bump()).collect();
        if hex.len() != digits {
            return Err(AssetContextError::Parse(
                "truncated unicode escape".to_owned(),
            ));
        }
        let code = u32::from_str_radix(&hex, 16)
            .map_err(|_| AssetContextError::Parse(format!("invalid unicode escape `{hex}`")))?;
        char::from_u32(code)
            .ok_or_else(|| AssetContextError::Parse(format!("invalid unicode scalar `{hex}`")))
    }

    fn parse_array(&mut self) -> Result<TomlValue, AssetContextError> {
        self.bump(); // '['
        let mut items = Vec::new();
        loop {
            self.skip_ws_and_comments(true);
            if self.peek() == Some(']') {
                self.bump();
                return Ok(TomlValue::Array(items));
            }
            if self.peek().is_none() {
                return Err(AssetContextError::Parse("unterminated array".to_owned()));
            }
            items.push(self.parse_value()?);
            self.skip_ws_and_comments(true);
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(']') => {
                    self.bump();
                    return Ok(TomlValue::Array(items));
                }
                _ => {
                    return Err(AssetContextError::Parse(
                        "expected `,` or `]` in array".to_owned(),
                    ));
                }
            }
        }
    }

    fn parse_inline_table(&mut self) -> Result<TomlValue, AssetContextError> {
        self.bump(); // '{'
        let mut map = BTreeMap::new();
        loop {
            self.skip_ws_and_comments(true);
            if self.peek() == Some('}') {
                self.bump();
                return Ok(TomlValue::Table(map));
            }
            if self.peek().is_none() {
                return Err(AssetContextError::Parse(
                    "unterminated inline table".to_owned(),
                ));
            }
            let keys = self.parse_key_path()?;
            self.skip_ws_and_comments(false);
            if self.bump() != Some('=') {
                return Err(AssetContextError::Parse(
                    "expected `=` in inline table".to_owned(),
                ));
            }
            let val = self.parse_value()?;
            insert_nested(&mut map, &keys, val)?;
            self.skip_ws_and_comments(true);
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some('}') => {
                    self.bump();
                    return Ok(TomlValue::Table(map));
                }
                _ => {
                    return Err(AssetContextError::Parse(
                        "expected `,` or `}` in inline table".to_owned(),
                    ));
                }
            }
        }
    }
}

fn parse_toml_section(src: &str) -> Result<BTreeMap<String, TomlValue>, AssetContextError> {
    let mut cursor = TomlCursor::new(src);
    let mut root = BTreeMap::new();
    let mut prefix: Vec<String> = Vec::new();

    loop {
        cursor.skip_ws_and_comments(true);
        let Some(c) = cursor.peek() else {
            break;
        };
        if c == '[' {
            let is_array = cursor.rest().starts_with("[[");
            cursor.pos += if is_array { 2 } else { 1 };
            let keys = cursor.parse_key_path()?;
            cursor.skip_ws_and_comments(false);
            let close = if is_array { "]]" } else { "]" };
            if !cursor.rest().starts_with(close) {
                return Err(AssetContextError::Parse(format!(
                    "expected `{close}` closing table header"
                )));
            }
            cursor.pos += close.len();
            if is_array && keys.as_slice() == ["asset"] {
                prefix.clear();
            } else if !is_array && keys.first().map(String::as_str) == Some("asset") {
                prefix = keys[1..].to_vec();
            } else {
                prefix = keys;
            }
            continue;
        }
        let mut keys = prefix.clone();
        keys.extend(cursor.parse_key_path()?);
        cursor.skip_ws_and_comments(false);
        if cursor.bump() != Some('=') {
            return Err(AssetContextError::Parse(format!(
                "expected `=` after key `{}`",
                keys.join(".")
            )));
        }
        let val = cursor.parse_value()?;
        insert_nested(&mut root, &keys, val)?;
        cursor.skip_ws_and_comments(false);
        if matches!(cursor.peek(), Some('\n' | '\r')) {
            cursor.bump();
        }
    }

    Ok(root)
}

fn insert_nested(
    map: &mut BTreeMap<String, TomlValue>,
    keys: &[String],
    value: TomlValue,
) -> Result<(), AssetContextError> {
    let Some((first, rest)) = keys.split_first() else {
        return Ok(());
    };
    if rest.is_empty() {
        map.insert(first.clone(), value);
        return Ok(());
    }
    let entry = map
        .entry(first.clone())
        .or_insert_with(|| TomlValue::Table(BTreeMap::new()));
    match entry {
        TomlValue::Table(sub) => insert_nested(sub, rest, value),
        _ => Err(AssetContextError::Parse(format!(
            "cannot assign dotted key `{}` under non-table value",
            keys.join(".")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_context_from_target_populates_builtins_and_applies_overrides() {
        let mut overrides = BTreeMap::new();
        overrides.insert("brand".to_owned(), vec!["acme".to_owned()]);
        let ctx = BuildContext::from_target(
            Some(Os::Ios),
            true,
            &["en".to_owned(), "de".to_owned(), "en".to_owned()],
            &overrides,
        );
        assert_eq!(ctx.get("platform"), Some(&["ios".to_owned()][..]));
        assert_eq!(ctx.get("formFactor"), Some(&["mobile".to_owned()][..]));
        assert_eq!(ctx.get("env"), Some(&["production".to_owned()][..]));
        assert_eq!(
            ctx.get("locale"),
            Some(&["en".to_owned(), "de".to_owned()][..])
        );
        assert_eq!(ctx.get("brand"), Some(&["acme".to_owned()][..]));
        assert_eq!(ctx.target_os(), Some(Os::Ios));

        let defines = ctx.define_constants();
        assert_eq!(
            defines
                .get("process.env.NSH_CONTEXT_BRAND")
                .map(String::as_str),
            Some("\"acme\"")
        );
        assert_eq!(
            defines
                .get("import.meta.env.NSH_CONTEXT_BRAND")
                .map(String::as_str),
            Some("\"acme\"")
        );
        assert_eq!(
            defines
                .get("process.env.NSH_CONTEXT_PLATFORM")
                .map(String::as_str),
            Some("\"ios\"")
        );
        assert_eq!(
            defines
                .get("process.env.NSH_CONTEXT_LOCALE")
                .map(String::as_str),
            Some("\"en,de\"")
        );
        assert_eq!(
            defines
                .get("process.env.NSH_CONTEXT_FORM_FACTOR")
                .map(String::as_str),
            Some("\"mobile\"")
        );
    }

    #[test]
    fn when_predicate_evaluates_and_or_and_negation() {
        let ios_acme = BuildContext::from_target(
            Some(Os::Ios),
            false,
            &["en".to_owned()],
            &BTreeMap::from([("brand".to_owned(), vec!["acme".to_owned()])]),
        );
        let android_acme = BuildContext::from_target(
            Some(Os::Android),
            false,
            &["en".to_owned()],
            &BTreeMap::from([("brand".to_owned(), vec!["acme".to_owned()])]),
        );
        let web_globex = BuildContext::from_target(
            Some(Os::Web),
            true,
            &["fr".to_owned()],
            &BTreeMap::from([("brand".to_owned(), vec!["globex".to_owned()])]),
        );

        // Empty predicate always matches.
        assert!(WhenPredicate::default().matches(&ios_acme));
        assert!(WhenPredicate::default().matches(&BuildContext::new()));

        // AND across keys, OR within array.
        let mut pred = WhenPredicate::default();
        pred.conditions.insert(
            "platform".to_owned(),
            vec!["ios".to_owned(), "macos".to_owned()],
        );
        pred.conditions
            .insert("brand".to_owned(), vec!["acme".to_owned()]);
        assert!(pred.matches(&ios_acme));
        assert!(!pred.matches(&android_acme));
        assert!(!pred.matches(&web_globex));

        // Negation (`!web`) excludes web and accepts ios/android.
        let mut not_web = WhenPredicate::default();
        not_web
            .conditions
            .insert("platform".to_owned(), vec!["!web".to_owned()]);
        assert!(not_web.matches(&ios_acme));
        assert!(not_web.matches(&android_acme));
        assert!(!not_web.matches(&web_globex));

        // Combined positive OR and negation.
        let mut mixed = WhenPredicate::default();
        mixed.conditions.insert(
            "brand".to_owned(),
            vec!["acme".to_owned(), "globex".to_owned(), "!globex".to_owned()],
        );
        assert!(mixed.matches(&ios_acme));
        assert!(!mixed.matches(&web_globex));
    }

    #[test]
    fn glob_and_exclude_filter_matching_files() {
        let toml = r#"
namespaces = ["icons/nav"]
files = ["fonts/**/*-OFL.txt"]

[[asset]]
path = "icons/**/*.{svg,png}"
exclude = ["icons/draft/**", "icons/**/temp-?.svg"]
kind = "icon"
namespace = "icons/{subdir}"
tags = ["ui", "nav"]
"#;
        let manifest = AssetFilterManifest::parse(toml).unwrap();
        let files: Vec<String> = [
            "assets.toml",
            "fonts/sans/Inter-OFL.txt",
            "icons/nav/home.svg",
            "icons/nav/badge.png",
            "icons/nav/temp-1.svg",
            "icons/draft/wip.svg",
            "other/readme.txt",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();

        let resolved =
            resolve_package_assets("acme-ui", &files, &manifest, &BuildContext::new()).unwrap();
        let included: Vec<&str> = resolved
            .included
            .iter()
            .map(|a| a.logical_path.as_str())
            .collect();
        assert_eq!(
            included,
            [
                "fonts/sans/Inter-OFL.txt",
                "icons/nav/badge.png",
                "icons/nav/home.svg",
                "other/readme.txt",
            ]
        );
        assert!(matches!(
            resolved.excluded.get("icons/draft/wip.svg"),
            Some(ExclusionReason::ExcludedByPattern(p)) if p == "icons/draft/**"
        ));
        assert!(matches!(
            resolved.excluded.get("icons/nav/temp-1.svg"),
            Some(ExclusionReason::ExcludedByPattern(p)) if p == "icons/**/temp-?.svg"
        ));
    }

    #[test]
    fn alias_resolution_and_pruned_manifest_preserve_unknown_metadata() {
        let toml = r#"
namespaces = ["brand", "platform"]

[[asset]]
path = "brand/acme/logo.svg"
alias = "brand/logo.svg"
kind = "illustration"
namespace = "brand"
title = "Acme Logo"
tags = ["brand", "header"]
width = 120
height = 40
when = { brand = "acme" }

[[asset]]
path = "brand/globex/logo.svg"
alias = "brand/logo.svg"
kind = "illustration"
namespace = "brand"
title = "Globex Logo"
when = { brand = "globex" }

[[asset]]
path = "ios/badge.svg"
kind = "icon"
namespace = "platform"
when = { platform = ["ios", "macos"] }
"#;
        let manifest = AssetFilterManifest::parse(toml).unwrap();
        let files: Vec<String> = [
            "brand/acme/logo.svg",
            "brand/globex/logo.svg",
            "ios/badge.svg",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();

        let acme_android = BuildContext::from_target(
            Some(Os::Android),
            true,
            &[],
            &BTreeMap::from([("brand".to_owned(), vec!["acme".to_owned()])]),
        );
        let resolved = resolve_package_assets("acme-ui", &files, &manifest, &acme_android).unwrap();

        assert_eq!(resolved.included.len(), 1);
        assert_eq!(resolved.included[0].logical_path, "brand/logo.svg");
        assert_eq!(resolved.included[0].physical_path, "brand/acme/logo.svg");
        assert_eq!(
            resolved.aliases.get("brand/logo.svg").map(String::as_str),
            Some("brand/acme/logo.svg")
        );
        assert!(resolved.excluded.contains_key("brand/globex/logo.svg"));
        assert!(resolved.excluded.contains_key("ios/badge.svg"));

        // Pruned manifest keeps top-level `namespaces` and surviving entry's
        // unknown metadata fields verbatim, dropping inactive entries.
        let pruned = &resolved.pruned_manifest;
        assert!(
            pruned.contains("namespaces = [\"brand\", \"platform\"]"),
            "{pruned}"
        );
        assert!(
            pruned.contains("path = \"brand/acme/logo.svg\""),
            "{pruned}"
        );
        assert!(pruned.contains("kind = \"illustration\""), "{pruned}");
        assert!(pruned.contains("title = \"Acme Logo\""), "{pruned}");
        assert!(
            pruned.contains("tags = [\"brand\", \"header\"]"),
            "{pruned}"
        );
        assert!(pruned.contains("width = 120"), "{pruned}");
        assert!(!pruned.contains("brand/globex/logo.svg"), "{pruned}");
        assert!(!pruned.contains("ios/badge.svg"), "{pruned}");
    }

    #[test]
    fn alias_conflict_between_two_active_entries_is_an_error() {
        let toml = r#"
[[asset]]
path = "brand/a/logo.svg"
alias = "brand/logo.svg"
when = { platform = "ios" }

[[asset]]
path = "brand/b/logo.svg"
alias = "brand/logo.svg"
when = { env = "production" }
"#;
        let manifest = AssetFilterManifest::parse(toml).unwrap();
        let files = vec!["brand/a/logo.svg".to_owned(), "brand/b/logo.svg".to_owned()];
        let ctx = BuildContext::from_target(Some(Os::Ios), true, &[], &BTreeMap::new());
        let err = resolve_package_assets("acme-ui", &files, &manifest, &ctx).unwrap_err();
        match err {
            AssetContextError::AliasConflict {
                package,
                alias,
                sources,
            } => {
                assert_eq!(package, "acme-ui");
                assert_eq!(alias, "brand/logo.svg");
                assert_eq!(sources, ["brand/a/logo.svg", "brand/b/logo.svg"]);
            }
            other => panic!("expected AliasConflict, got {other:?}"),
        }
    }

    #[test]
    fn subtable_when_and_density_sibling_aliases_work() {
        let toml = r#"
[[asset]]
path = "brand/acme/hero.png"
alias = "brand/hero.png"
[asset.when]
brand = "acme"
"#;
        let manifest = AssetFilterManifest::parse(toml).unwrap();
        let files = vec![
            "brand/acme/hero.png".to_owned(),
            "brand/acme/hero@2x.png".to_owned(),
        ];
        let ctx = BuildContext::new().with("brand", "acme");
        let resolved = resolve_package_assets("acme-ui", &files, &manifest, &ctx).unwrap();
        let paths: Vec<(&str, &str)> = resolved
            .included
            .iter()
            .map(|a| (a.logical_path.as_str(), a.physical_path.as_str()))
            .collect();
        assert_eq!(
            paths,
            [
                ("brand/hero.png", "brand/acme/hero.png"),
                ("brand/hero@2x.png", "brand/acme/hero@2x.png"),
            ]
        );
    }
}
