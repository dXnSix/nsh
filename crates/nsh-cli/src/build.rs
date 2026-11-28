use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::Context as _;
use nsh_bundler::{AppInfo, BundleError, BundleOptions};
use nsh_core::element::{AssetFilterManifest, BuildContext, resolve_package_assets};

use crate::ui;

/// A failed build, formatted for terminals and for device error screens.
#[derive(Debug)]
pub(crate) struct BuildError {
    pub(crate) colored: String,
    pub(crate) plain: String,
}

impl From<BundleError> for BuildError {
    fn from(err: BundleError) -> Self {
        Self {
            colored: err.colored,
            plain: err.plain,
        }
    }
}

impl From<nsh_js::JsError> for BuildError {
    fn from(err: nsh_js::JsError) -> Self {
        Self {
            colored: err.message.clone(),
            plain: err.message,
        }
    }
}

/// A compiled app, in memory.
pub(crate) struct Built {
    /// The bundle as JS source.
    pub(crate) code: String,
    /// The bundle as QuickJS bytecode.
    pub(crate) bytecode: Vec<u8>,
    /// Source map of `code`.
    pub(crate) source_map: Option<String>,
    /// Shared and lazy chunks for a split build ([`compile_split`]).
    pub(crate) chunks: Vec<BuiltChunk>,
    /// Static chunks required by the entry, including transitive imports.
    pub(crate) entry_imports: Vec<String>,
    /// The files the app ships: those it imports and those its UI sets'
    /// crates ship ([`crate_assets`]).
    pub(crate) assets: Vec<Asset>,
    /// The directory its imported assets are named from
    /// (`nsh_bundler::Bundle::asset_root`), where the dev server finds the
    /// media files a literal `asset:/` URL names; `None` for a Rust app.
    pub(crate) asset_root: Option<PathBuf>,
    /// Files of the last build, including shared sources outside the app.
    pub(crate) watch_files: Vec<PathBuf>,
    /// The directories outside the app its `import.meta.glob` patterns
    /// search (`nsh_bundler::Bundle::glob_dirs`).
    pub(crate) glob_dirs: Vec<nsh_bundler::GlobDir>,
    /// Every `import.meta.glob` pattern it searched, in the app or outside
    /// it (`nsh_bundler::Bundle::glob_patterns`).
    pub(crate) glob_patterns: Vec<nsh_bundler::GlobDir>,
    /// What the app declares it may reach (`nsh.json` `capabilities`).
    pub(crate) capabilities: Option<nsh_core::access::Capabilities>,
    /// Checked URL claims, including fileTypes-derived claims, from the bundler.
    pub(crate) url_schemes: Vec<String>,
    /// The UI sets it imports, by specifier ([`imported_sets`]).
    pub(crate) ui_sets: Vec<String>,
    /// What its code uses of the native modules, by export
    /// (`nsh_bundler::Bundle::native_use`); `None` when unknown (a Rust
    /// app's, a development build's).
    pub(crate) native_use: Option<nsh_bundler::NativeUse>,
    /// What the entry chunk's own code uses of them
    /// (`nsh_bundler::Bundle::entry_native_use`); `None` when unknown, and
    /// then the chunks' are not used either: everything is the entry's.
    pub(crate) entry_native_use: Option<nsh_bundler::NativeUse>,
    /// How long bundling and compiling took.
    pub(crate) elapsed: Duration,
    /// A Rust app (`nsh.json` `"root"`, [`rust_root`]): no bundle, its
    /// runners mount its root.
    pub(crate) rust_root: bool,
}

/// What a Rust app (`nsh.json` `"root"`) ships besides its runner: the
/// files its crates register (no bundle: the runner mounts the root).
pub(crate) fn rust_root() -> Built {
    Built {
        code: String::new(),
        bytecode: Vec::new(),
        source_map: None,
        chunks: Vec::new(),
        entry_imports: Vec::new(),
        assets: crate_assets().collect(),
        asset_root: None,
        watch_files: Vec::new(),
        glob_dirs: Vec::new(),
        glob_patterns: Vec::new(),
        capabilities: None,
        url_schemes: Vec::new(),
        ui_sets: Vec::new(),
        native_use: None,
        entry_native_use: None,
        elapsed: Duration::ZERO,
        rust_root: true,
    }
}

/// A chunk of a split build: app code `import()` loads on demand.
pub(crate) struct BuiltChunk {
    /// Its id (`Screen.js`): the name the app's code imports it by, stable
    /// while the app's modules keep their names. Packagers map it to the
    /// chunk's file or URL.
    pub(crate) id: String,
    /// The chunk as JS source (an ES module).
    pub(crate) code: String,
    /// The chunk as QuickJS bytecode.
    pub(crate) bytecode: Vec<u8>,
    /// Source map of `code`.
    pub(crate) source_map: Option<String>,
    /// What its own code uses of the native modules
    /// (`nsh_bundler::Bundle::chunk_native_use`); `None` when unknown.
    pub(crate) native_use: Option<nsh_bundler::NativeUse>,
}

/// A file an app ships.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Asset {
    /// Its path among the app's assets (`asset:/<path>`), `/`-separated.
    pub(crate) path: String,
    /// The file.
    pub(crate) source: PathBuf,
    /// The targets it is for; none: every target
    /// (`Registry::assets_for`).
    pub(crate) targets: Vec<nsh_core::platform::Os>,
}

/// The files the UI sets' crates ship (`Registry::assets`), published
/// under `@<package>/`.
pub(crate) fn crate_assets() -> impl Iterator<Item = Asset> {
    let assets: Vec<Asset> = crate::ui_sets()
        .iter()
        .flat_map(|set| &set.assets)
        .map(|file| Asset {
            path: file.path.clone(),
            source: PathBuf::from(&file.source),
            targets: file.targets.clone(),
        })
        .collect();
    assets.into_iter()
}

/// The native modules of `natives` that are UI sets (`nsh/base`, an
/// app's own): what a host must serve besides nsh's own modules, which
/// every host has.
fn imported_sets(natives: &[String]) -> Vec<String> {
    natives
        .iter()
        .filter(|spec| crate::ui_sets().iter().any(|set| set.specifier == **spec))
        .cloned()
        .collect()
}

/// The app's assets: `imported` (relative to the app's directory `root`,
/// the bundle's `asset_root`) and the UI sets' ([`crate_assets`]).
fn assets(root: &Path, imported: &[PathBuf]) -> Vec<Asset> {
    let mut assets: Vec<Asset> = imported
        .iter()
        .map(|rel| Asset {
            path: rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/"),
            source: root.join(rel),
            targets: Vec::new(),
        })
        .chain(crate_assets())
        .collect();
    assets.sort_by(|a, b| a.path.cmp(&b.path));
    assets.dedup_by(|a, b| a.path == b.path);
    assets
}

/// A file to ship in a packaged app or serve from the dev server: either
/// copied from disk or generated in memory (such as a pruned `assets.toml`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackagedAssetContent {
    /// Physical file on disk.
    File(PathBuf),
    /// Generated bytes (for example a pruned `assets.toml` manifest).
    Bytes(Vec<u8>),
}

/// One resolved asset ready to be written into a build output or package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagedAsset {
    /// Resolved logical path among the app's assets (`asset:/<path>`),
    /// `/`-separated.
    pub(crate) path: String,
    /// File source or generated bytes.
    pub(crate) content: PackagedAssetContent,
}

impl PackagedAsset {
    /// Writes this asset into `assets_dir/<path>`, creating parent directories.
    pub(crate) fn write_to(&self, assets_dir: &Path) -> anyhow::Result<()> {
        let dest = assets_dir.join(&self.path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        match &self.content {
            PackagedAssetContent::File(source) => {
                fs::copy(source, &dest).with_context(|| format!("copy {}", self.path))?;
            }
            PackagedAssetContent::Bytes(bytes) => {
                fs::write(&dest, bytes).with_context(|| format!("write {}", self.path))?;
            }
        }
        Ok(())
    }

    /// Returns the bytes of this asset (reading from disk or cloning memory).
    pub(crate) fn read_bytes(&self) -> anyhow::Result<Vec<u8>> {
        match &self.content {
            PackagedAssetContent::File(source) => {
                fs::read(source).with_context(|| format!("read {}", self.path))
            }
            PackagedAssetContent::Bytes(bytes) => Ok(bytes.clone()),
        }
    }
}

/// Splits `@<package>/<rel>` into `(<package>, <rel>)`.
fn split_package_asset_path(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix('@')?;
    let (pkg, rel) = rest.split_once('/')?;
    (!pkg.is_empty() && !rel.is_empty()).then_some((pkg, rel))
}

/// Filters and alias-resolves `assets` for `ctx`, emitting pruned `assets.toml`
/// manifests for packages that declare one.
pub(crate) fn resolve_assets_for_context(
    assets: &[Asset],
    ctx: &BuildContext,
) -> anyhow::Result<Vec<PackagedAsset>> {
    let target = ctx.target_os();
    let mut by_package: BTreeMap<String, BTreeMap<String, PathBuf>> = BTreeMap::new();
    let mut out: Vec<PackagedAsset> = Vec::new();

    for asset in assets {
        if !asset.targets.is_empty() && target.is_some_and(|t| !asset.targets.contains(&t)) {
            continue;
        }
        if let Some((pkg, rel)) = split_package_asset_path(&asset.path) {
            by_package
                .entry(pkg.to_owned())
                .or_default()
                .insert(rel.to_owned(), asset.source.clone());
        } else {
            out.push(PackagedAsset {
                path: asset.path.clone(),
                content: PackagedAssetContent::File(asset.source.clone()),
            });
        }
    }

    for (pkg, pkg_files) in by_package {
        let manifest = match pkg_files.get("assets.toml") {
            Some(source) => match fs::read_to_string(source) {
                Ok(text) => Some(
                    AssetFilterManifest::parse(&text)
                        .with_context(|| format!("parse @{pkg}/assets.toml"))?,
                ),
                Err(_) => None,
            },
            None => None,
        };

        if let Some(manifest) = manifest {
            let rel_list: Vec<String> = pkg_files.keys().cloned().collect();
            let resolved = resolve_package_assets(&pkg, &rel_list, &manifest, ctx)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            for item in resolved.included {
                if let Some(source) = pkg_files.get(&item.physical_path) {
                    out.push(PackagedAsset {
                        path: format!("@{pkg}/{}", item.logical_path),
                        content: PackagedAssetContent::File(source.clone()),
                    });
                }
            }
            out.push(PackagedAsset {
                path: format!("@{pkg}/assets.toml"),
                content: PackagedAssetContent::Bytes(resolved.pruned_manifest.into_bytes()),
            });
        } else {
            for (rel, source) in pkg_files {
                out.push(PackagedAsset {
                    path: format!("@{pkg}/{rel}"),
                    content: PackagedAssetContent::File(source),
                });
            }
        }
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    out.dedup_by(|a, b| a.path == b.path);
    Ok(out)
}

impl Built {
    /// The files the package for `target` ships: the app's own, and the
    /// crates' of every target or of `target` ([`Registry::assets_for`]).
    /// No `target`: all of them.
    ///
    /// [`Registry::assets_for`]: nsh_core::element::Registry::assets_for
    #[allow(dead_code)]
    pub(crate) fn assets_for(
        &self,
        target: Option<nsh_core::platform::Os>,
    ) -> impl Iterator<Item = &Asset> {
        self.assets.iter().filter(move |asset| {
            asset.targets.is_empty() || target.is_none_or(|target| asset.targets.contains(&target))
        })
    }

    /// The files the package for `ctx` ships: target-scoped, `when`-filtered,
    /// alias-resolved, with pruned `assets.toml` manifests for packages that
    /// declare one.
    pub(crate) fn assets_for_context(
        &self,
        ctx: &BuildContext,
    ) -> anyhow::Result<Vec<PackagedAsset>> {
        resolve_assets_for_context(&self.assets, ctx)
    }

    /// `capabilities.json` (written next to the bundle), when the app
    /// declares capabilities.
    pub(crate) fn capabilities_json(&self) -> Option<String> {
        self.capabilities.as_ref().map(|c| c.to_json().to_string())
    }

    /// `(3.2 KB js, 9.8 KB bytecode<, 2 chunks><more>, 21ms)`, dimmed.
    pub(crate) fn summary(&self, more: &str) -> String {
        let chunks = match self.chunks.len() {
            0 => String::new(),
            1 => ", 1 chunk".to_owned(),
            n => format!(", {n} chunks"),
        };
        ui::dim(format!(
            "({} js, {} bytecode{chunks}{more}, {})",
            ui::bytes(self.code.len()),
            ui::bytes(self.bytecode.len()),
            ui::duration(self.elapsed)
        ))
    }
}

impl BuildError {
    /// The error for a command that ends with it.
    pub(crate) fn into_error(self, entry: &Path) -> anyhow::Error {
        ui::failure(format!("building {} failed", ui::path(entry)), self.colored)
    }
}

/// Bundles `entry` and compiles it to bytecode: one bundle, holding what
/// `import()` loads too (native packages).
pub(crate) fn compile(entry: &Path, release: bool) -> Result<Built, BuildError> {
    compile_with(
        entry,
        release,
        false,
        release.into(),
        AppInfo::default(),
        None,
    )
}

/// [`compile`] with an explicit [`BuildContext`].
pub(crate) fn compile_with_context(
    entry: &Path,
    release: bool,
    split: bool,
    context: Option<&BuildContext>,
) -> Result<Built, BuildError> {
    compile_with(
        entry,
        release,
        split,
        release.into(),
        AppInfo::default(),
        context,
    )
}

/// [`compile`] in release mode for a package of `config`'s app (`split`:
/// as [`compile_split`] does, for web apps), giving it `config`'s version
/// and build number (`--version`, `--build`), with `context`.
pub(crate) fn compile_package_with_context(
    entry: &Path,
    split: bool,
    config: &crate::package::AppConfig,
    context: &BuildContext,
) -> Result<Built, BuildError> {
    compile_with(
        entry,
        true,
        split,
        true.into(),
        app_info(entry, config),
        Some(context),
    )
}

/// [`compile`] in release mode (minified), with the bytecode stripped as
/// `strip` says (signed packages keep their line tables by default), giving
/// the app `config`'s version and build number.
pub(crate) fn compile_release(
    entry: &Path,
    strip: nsh_js::Strip,
    config: &crate::package::AppConfig,
) -> Result<Built, BuildError> {
    compile_with(entry, true, false, strip, app_info(entry, config), None)
}

/// The identity a package of `config`'s app carries (after `--id`,
/// `--name`, `--version` and `--build`, with the defaults the package
/// writers use: [`crate::package::name_and_id`], `1.0.0`, build `1`), so
/// `appInfo()` reports what the package says it is.
pub(crate) fn app_info(entry: &Path, config: &crate::package::AppConfig) -> AppInfo {
    let (name, id) = crate::package::name_and_id(entry, config);
    let mut info = AppInfo::default();
    info.id = Some(id);
    info.name = Some(name);
    info.version = Some(config.version.clone().unwrap_or_else(|| "1.0.0".to_owned()));
    info.build = Some(u64::from(config.build.unwrap_or(1)));
    info
}

fn compile_with(
    entry: &Path,
    release: bool,
    split: bool,
    strip: nsh_js::Strip,
    app: AppInfo,
    context: Option<&BuildContext>,
) -> Result<Built, BuildError> {
    let start = Instant::now();
    let mut bundle = nsh_bundler::bundle(&BundleOptions {
        entry: entry.to_path_buf(),
        minify: release,
        development: !release,
        split,
        ui_sets: crate::ui_sets().to_vec(),
        app,
        asset_root_from: None,
        app_dir: crate::entry::app_dir_override().map(Path::to_path_buf),
        context: context.cloned(),
    })?;
    let sources: Vec<(&str, &str)> = std::iter::once((nsh_js::BUNDLE_NAME, bundle.code.as_str()))
        .chain(
            bundle
                .chunks
                .iter()
                .map(|c| (c.id.as_str(), c.code.as_str())),
        )
        .collect();
    let mut compiled =
        nsh_js::compile_chunk_bytecode(&bundle.code, &sources, strip, &crate::ui_sets())?
            .into_iter();
    let bytecode = compiled.next().expect("entry bytecode");
    let mut chunk_native_use = std::mem::take(&mut bundle.chunk_native_use);
    let chunks = bundle
        .chunks
        .into_iter()
        .zip(compiled)
        .map(|(c, bytecode)| BuiltChunk {
            native_use: chunk_native_use.remove(&c.id),
            id: c.id,
            code: c.code,
            bytecode,
            source_map: c.source_map,
        })
        .collect();
    Ok(Built {
        code: bundle.code,
        bytecode,
        source_map: bundle.source_map,
        chunks,
        entry_imports: bundle.entry_imports,
        assets: assets(&bundle.asset_root, &bundle.assets),
        asset_root: Some(bundle.asset_root),
        watch_files: bundle.watch_files,
        glob_dirs: bundle.glob_dirs,
        glob_patterns: bundle.glob_patterns,
        capabilities: bundle.capabilities,
        url_schemes: bundle.links.schemes().to_vec(),
        ui_sets: imported_sets(&bundle.natives),
        native_use: Some(bundle.native_use),
        entry_native_use: Some(bundle.entry_native_use),
        elapsed: start.elapsed(),
        rust_root: false,
    })
}

/// Bundles `entry` for development with hot module replacement: the whole
/// app (source and bytecode) plus the modules, to compute later updates.
#[cfg(test)]
pub(crate) fn compile_hot(entry: &Path) -> Result<(Built, nsh_bundler::HotBundle), BuildError> {
    compile_hot_with_context(entry, None)
}

/// [`compile_hot`] with an explicit [`BuildContext`].
pub(crate) fn compile_hot_with_context(
    entry: &Path,
    context: Option<&BuildContext>,
) -> Result<(Built, nsh_bundler::HotBundle), BuildError> {
    let start = Instant::now();
    let hot = nsh_bundler::bundle_hot_in_with_context(
        entry,
        crate::entry::app_dir_override(),
        &crate::ui_sets(),
        context,
    )?;
    let (code, source_map) = hot.script();
    let bytecode = nsh_js::compile_bytecode_with(&code, nsh_js::Strip::Source, &crate::ui_sets())?;
    let built = Built {
        code,
        bytecode,
        source_map,
        chunks: Vec::new(),
        entry_imports: Vec::new(),
        assets: assets(&hot.asset_root, &hot.assets),
        asset_root: Some(hot.asset_root.clone()),
        watch_files: hot.watch_files.clone(),
        glob_dirs: hot.glob_dirs.clone(),
        glob_patterns: hot.glob_patterns.clone(),
        capabilities: hot.capabilities.clone(),
        url_schemes: hot.links.schemes().to_vec(),
        ui_sets: imported_sets(hot.natives()),
        native_use: None,
        entry_native_use: None,
        elapsed: start.elapsed(),
        rust_root: false,
    };
    Ok((built, hot))
}

/// Bundles `entry` into `out/bundle.js`, `out/bundle.js.map` and
/// `out/bundle.nshc`; `split`: with each chunk next to them
/// (`Screen.js`, `Screen.js.map`, `Screen.nshc`).
/// A Rust app (`nsh.json` `"root"`, its crates built before this command
/// line ran) writes only `app.json` and its crates' assets: its runners
/// mount the root.
pub(crate) fn build(
    entry: &Path,
    out: &Path,
    release: bool,
    split: bool,
    context: &BuildContext,
) -> anyhow::Result<()> {
    let built = if crate::rust_app::is_root(entry) {
        rust_root()
    } else {
        compile_with_context(entry, release, split, Some(context))
            .map_err(|e| e.into_error(entry))?
    };
    fs::create_dir_all(out).with_context(|| format!("create {}", out.display()))?;
    let map = built.source_map.as_deref().unwrap_or_default();
    let mut files = vec![
        ("bundle.js".to_owned(), built.code.as_bytes()),
        ("bundle.js.map".to_owned(), map.as_bytes()),
        ("bundle.nshc".to_owned(), &built.bytecode[..]),
    ];
    if built.rust_root {
        // No bundle; one left from a TSX entry would not be this app.
        for (name, _) in files.drain(..) {
            let _ = fs::remove_file(out.join(name));
        }
    }
    for chunk in &built.chunks {
        let stem = chunk.id.strip_suffix(".js").unwrap_or(&chunk.id);
        let map = chunk.source_map.as_deref().unwrap_or_default();
        files.push((chunk.id.clone(), chunk.code.as_bytes()));
        files.push((format!("{}.map", chunk.id), map.as_bytes()));
        files.push((format!("{stem}.nshc"), &chunk.bytecode[..]));
    }
    for (name, bytes) in files {
        let path = out.join(name);
        fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
    }
    // The app's id, which names where it keeps its data, and the links it
    // claims (`Bundle::read`).
    let app = out.join("app.json");
    let id = crate::package::app_id(entry)?;
    let config = crate::package::read_config(&dunce::canonicalize(entry)?)?;
    let links = config.app_links();
    fs::write(
        &app,
        crate::package::app_json(
            {
                let mut info = serde_json::json!({ "id": id });
                config.add_file_types_to_app_json(&mut info);
                config.add_data_dir_to_app_json(&mut info);
                if let Some(application) = &config.application {
                    info["application"] = application.clone();
                }
                info
            },
            &links,
            config.window.as_ref(),
        ),
    )
    .with_context(|| format!("write {}", app.display()))?;
    let declared = out.join("capabilities.json");
    match built.capabilities_json() {
        Some(json) => {
            fs::write(&declared, json).with_context(|| format!("write {}", declared.display()))?
        }
        None => {
            let _ = fs::remove_file(&declared);
        }
    }
    // Assets keep their resolved logical paths: `asset:/img/a.png` -> `out/assets/img/a.png`.
    let packaged_assets = built.assets_for_context(context)?;
    let assets_dir = out.join("assets");
    for asset in &packaged_assets {
        asset.write_to(&assets_dir)?;
    }
    let assets = match packaged_assets.len() {
        0 => String::new(),
        1 => ", 1 asset".to_owned(),
        n => format!(", {n} assets"),
    };
    if built.rust_root {
        ui::done(format!(
            "built the Rust app {} into {} {}",
            ui::path(entry),
            ui::path(out),
            ui::dim(format!("(no bundle: its runners mount the root{assets})"))
        ));
    } else {
        ui::done(format!(
            "built {} into {} {}",
            ui::path(entry),
            ui::path(out),
            built.summary(&assets)
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// Separates development bundling, hot-script assembly and QuickJS compilation.
    /// Run with `cargo test -p nsh-cli bench_dev_startup -- --ignored --nocapture`.
    #[test]
    #[ignore = "startup measurement on the counter and gallery"]
    fn bench_dev_startup() {
        use std::time::Instant;

        let sets = crate::ui_sets();
        for app in ["counter", "gallery"] {
            let entry = PathBuf::from(format!("../../examples/{app}/App.tsx"));
            for run in 0..3 {
                let start = Instant::now();
                let hot = nsh_bundler::bundle_hot(&entry, &sets).unwrap();
                let bundled = start.elapsed();
                let start = Instant::now();
                let (code, _) = hot.script();
                let assembled = start.elapsed();
                let start = Instant::now();
                let bytecode =
                    nsh_js::compile_bytecode_with(&code, nsh_js::Strip::Source, &sets).unwrap();
                let compiled = start.elapsed();
                assert!(!bytecode.is_empty());
                // What keeping the functions' source text would cost.
                let unstripped = nsh_js::compile_bytecode_with(&code, false, &sets).unwrap();
                eprintln!(
                    "{app} run {run}: bundle {bundled:?}, script {assembled:?}, bytecode {compiled:?}; {} source bytes, {} bytecode bytes ({} with function source)",
                    code.len(),
                    bytecode.len(),
                    unstripped.len()
                );
            }
        }
    }

    #[test]
    fn hosts_must_serve_the_ui_sets_an_app_imports() {
        let natives = ["nsh", "nsh/mobile", "nsh/base"].map(String::from);
        assert_eq!(super::imported_sets(&natives), ["nsh/base"]);
    }

    #[test]
    fn imported_assets_keep_their_paths_under_the_app() {
        let assets = super::assets(
            Path::new("/app"),
            &[PathBuf::from("img/a.png"), PathBuf::from("b.ttf")],
        );
        let paths: Vec<_> = assets
            .iter()
            .map(|a| (a.path.as_str(), a.source.clone()))
            .collect();
        assert_eq!(
            paths,
            [
                ("b.ttf", PathBuf::from("/app/b.ttf")),
                ("img/a.png", PathBuf::from("/app/img/a.png")),
            ]
        );
    }

    #[test]
    fn a_package_ships_the_files_of_its_target() {
        use nsh_core::platform::Os;
        let file = |path: &str, targets: &[Os]| crate::build::Asset {
            path: path.to_owned(),
            source: PathBuf::from(path),
            targets: targets.to_vec(),
        };
        let built = crate::build::Built {
            assets: vec![
                file("all.png", &[]),
                file("web.json", &[Os::Web]),
                file("phones.json", &[Os::Ios, Os::Android]),
            ],
            ..crate::build::rust_root()
        };
        let paths = |target| {
            built
                .assets_for(target)
                .map(|a| a.path.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(paths(Some(Os::Web)), ["all.png", "web.json"]);
        assert_eq!(paths(Some(Os::MacOs)), ["all.png"]);
        assert_eq!(paths(Some(Os::Ios)), ["all.png", "phones.json"]);
    }

    /// An entry below its app (`tests/SubApp.tsx`, which a test opens)
    /// ships the files it imports from anywhere in the app, named from the
    /// app's directory as the app's own entry names them (#1483).
    #[test]
    fn an_entry_below_its_app_ships_assets_named_from_the_app() {
        let dir = std::env::temp_dir().join(format!("nsh-cli-sub-assets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::create_dir_all(dir.join("tests")).unwrap();
        std::fs::write(dir.join("nsh.json"), "{}").unwrap();
        std::fs::write(dir.join("App.tsx"), "export {};\n").unwrap();
        // A PNG's header: all the bundler reads of it.
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(4u32.to_be_bytes());
        png.extend(4u32.to_be_bytes());
        png.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
        std::fs::write(dir.join("assets/dot.png"), &png).unwrap();
        std::fs::write(
            dir.join("tests/SubApp.tsx"),
            "import { Image, mount } from \"nsh\";\nimport dot from \"../assets/dot.png\";\nmount(() => <Image src={dot} />);\n",
        )
        .unwrap();
        let root = dunce::canonicalize(&dir).unwrap();
        let check = |built: &super::Built| {
            assert!(
                built.code.contains("\"asset:/assets/dot.png\""),
                "{}",
                built.code
            );
            let asset = built
                .assets
                .iter()
                .find(|a| a.path == "assets/dot.png")
                .expect("the image ships");
            assert_eq!(asset.source, root.join("assets/dot.png"));
            assert!(asset.source.is_file());
            assert_eq!(built.asset_root.as_deref(), Some(root.as_path()));
        };
        let entry = dir.join("tests/SubApp.tsx");
        check(&super::compile(&entry, false).unwrap());
        check(&super::compile_hot(&entry).unwrap().0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn assets_for_context_prunes_by_platform_and_brand_and_emits_pruned_manifest() {
        use std::collections::BTreeMap;

        use nsh_core::{element::BuildContext, platform::Os};

        let dir =
            std::env::temp_dir().join(format!("nsh-cli-asset-context-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let pkg_dir = dir.join("acme-ui/assets");
        for (rel, data) in [
            (
                "assets.toml",
                r#"
namespaces = ["brand", "platform"]

[[asset]]
path = "brand/acme/logo.svg"
alias = "brand/logo.svg"
kind = "illustration"
title = "Acme Logo"
when = { brand = "acme" }

[[asset]]
path = "brand/globex/logo.svg"
alias = "brand/logo.svg"
kind = "illustration"
title = "Globex Logo"
when = { brand = "globex" }

[[asset]]
path = "ios/only.svg"
kind = "icon"
when = { platform = "ios" }

[[asset]]
path = "android/only.svg"
kind = "icon"
when = { platform = "android" }
"#
                .as_bytes(),
            ),
            ("brand/acme/logo.svg", b"<svg/>"),
            ("brand/globex/logo.svg", b"<svg/>"),
            ("ios/only.svg", b"<svg/>"),
            ("android/only.svg", b"<svg/>"),
        ] {
            let full = pkg_dir.join(rel);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(&full, data).unwrap();
        }

        let make_asset = |rel: &str| crate::build::Asset {
            path: format!("@acme-ui/{rel}"),
            source: pkg_dir.join(rel),
            targets: Vec::new(),
        };
        let built = crate::build::Built {
            assets: vec![
                make_asset("assets.toml"),
                make_asset("brand/acme/logo.svg"),
                make_asset("brand/globex/logo.svg"),
                make_asset("ios/only.svg"),
                make_asset("android/only.svg"),
            ],
            ..crate::build::rust_root()
        };

        let acme_ios = BuildContext::from_target(
            Some(Os::Ios),
            true,
            &[],
            &BTreeMap::from([("brand".to_owned(), vec!["acme".to_owned()])]),
        );
        let ios_assets = built.assets_for_context(&acme_ios).unwrap();
        let ios_paths: Vec<&str> = ios_assets.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(
            ios_paths,
            [
                "@acme-ui/assets.toml",
                "@acme-ui/brand/logo.svg",
                "@acme-ui/ios/only.svg",
            ]
        );
        let ios_logo = ios_assets
            .iter()
            .find(|a| a.path == "@acme-ui/brand/logo.svg")
            .unwrap();
        assert_eq!(
            ios_logo.content,
            crate::build::PackagedAssetContent::File(pkg_dir.join("brand/acme/logo.svg"))
        );
        let ios_toml = String::from_utf8(
            ios_assets
                .iter()
                .find(|a| a.path == "@acme-ui/assets.toml")
                .unwrap()
                .read_bytes()
                .unwrap(),
        )
        .unwrap();
        assert!(ios_toml.contains("title = \"Acme Logo\""), "{ios_toml}");
        assert!(ios_toml.contains("ios/only.svg"), "{ios_toml}");
        assert!(!ios_toml.contains("Globex Logo"), "{ios_toml}");
        assert!(!ios_toml.contains("android/only.svg"), "{ios_toml}");

        let globex_android = BuildContext::from_target(
            Some(Os::Android),
            true,
            &[],
            &BTreeMap::from([("brand".to_owned(), vec!["globex".to_owned()])]),
        );
        let android_assets = built.assets_for_context(&globex_android).unwrap();
        let android_paths: Vec<&str> = android_assets.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(
            android_paths,
            [
                "@acme-ui/android/only.svg",
                "@acme-ui/assets.toml",
                "@acme-ui/brand/logo.svg",
            ]
        );
        let android_logo = android_assets
            .iter()
            .find(|a| a.path == "@acme-ui/brand/logo.svg")
            .unwrap();
        assert_eq!(
            android_logo.content,
            crate::build::PackagedAssetContent::File(pkg_dir.join("brand/globex/logo.svg"))
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
