use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use oxc::span::SourceType;
use rolldown::{
    ModuleType,
    plugin::{
        HookLoadArgs, HookLoadOutput, HookLoadReturn, HookRenderChunkArgs, HookRenderChunkOutput,
        HookRenderChunkReturn, HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn,
        HookTransformArgs, HookTransformOutput, HookTransformOutputMap, HookTransformReturn,
        HookUsage, Plugin, PluginContext, SharedLoadPluginContext, SharedTransformPluginContext,
    },
};

use crate::compile::{NativeModules, compile_native};

/// Compiles the app's modules with nsh's JSX compiler against the native
/// modules (kept as imports) and loads its assets.
#[derive(Debug)]
pub(crate) struct NshPlugin {
    /// Paths in error messages are shown relative to this directory.
    pub(crate) root: PathBuf,
    /// The first compile error, already formatted. Rolldown wraps plugin
    /// errors in its own report; the bundler shows this one instead.
    pub(crate) error: Arc<Mutex<Option<String>>>,
    /// Asset files imported by the app (absolute paths).
    pub(crate) assets: Arc<Mutex<Vec<PathBuf>>>,
    /// Development bundle with hot module replacement: components and
    /// contexts of app modules are registered with the hot runtime.
    pub(crate) hot: bool,
    /// The app's themes (`nsh.json`).
    pub(crate) tailwind: Arc<ThemeSetup>,
    /// The entry module (it sets up the themes).
    pub(crate) entry: PathBuf,
    /// The native modules the app imports.
    pub(crate) natives: Arc<Mutex<std::collections::BTreeSet<String>>>,
    /// The native modules there are.
    pub(crate) modules: Arc<NativeModules>,
    /// A split bundle: dynamic imports of chunks call the runtime's chunk
    /// loader (`split.rs`).
    pub(crate) split: bool,
}

/// The `"tailwind"` and `"theme"` of the app's `nsh.json`.
#[derive(Debug, Default)]
pub(crate) struct ThemeSetup {
    /// The Tailwind theme as JSON, for the runtime (`None`: the default).
    pub(crate) json: Option<String>,
    /// Parsed, to check classes at build time.
    pub(crate) theme: nsh_core::tailwind::Theme,
    /// The theme of the Rust components (a gpui-kit theme file), checked.
    pub(crate) ui: Option<String>,
    /// `"appearance"`: `"light"` or `"dark"` (`None`: the system's).
    pub(crate) appearance: Option<&'static str>,
}

/// File types imported as image assets.
pub(crate) const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "heic", "svg"];

/// File types imported as fonts.
pub(crate) const FONT_EXTENSIONS: &[&str] = &["ttf", "otf"];

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// The family name a font registers under: the typographic family (name
/// 16) when present, else the legacy family (name 1).
fn font_family(data: &[u8]) -> Result<String, String> {
    let face = ttf_parser::Face::parse(data, 0).map_err(|e| e.to_string())?;
    let name = |id| {
        face.names()
            .into_iter()
            .filter(|n| n.name_id == id && n.is_unicode())
            .find_map(|n| n.to_string())
    };
    name(ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
        .or_else(|| name(ttf_parser::name_id::FAMILY))
        .ok_or_else(|| "the font has no family name".to_owned())
}

/// An imported SVG's size in points, as the native backends size it
/// ([`nsh_core::svg::size`]), and what of it shows in browsers only
/// ([`svg_browser_only`]).
fn svg_asset(data: &[u8]) -> Result<((f32, f32), Vec<&'static str>), String> {
    let size = nsh_core::svg::size(data).map_err(|e| e.to_string())?;
    Ok((size, svg_browser_only(data)))
}

/// What an SVG holds that browsers draw and the native backends'
/// rasterizer leaves out: text, animation and embedded images.
fn svg_browser_only(data: &[u8]) -> Vec<&'static str> {
    let Ok(text) = std::str::from_utf8(data) else {
        return Vec::new();
    };
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let Ok(doc) = roxmltree::Document::parse_with_options(text, options) else {
        return Vec::new();
    };
    let (mut texts, mut animation, mut images) = (false, false, false);
    let animated_css = |css: &str| css.contains("@keyframes") || css.contains("animation");
    for node in doc.descendants().filter(|n| n.is_element()) {
        match node.tag_name().name() {
            "text" => texts = true,
            "animate" | "animateMotion" | "animateTransform" | "set" => animation = true,
            "image" => images = true,
            "style" => {
                animation |= node
                    .descendants()
                    .filter_map(|n| n.text())
                    .any(animated_css)
            }
            _ => {}
        }
        animation |= node.attribute("style").is_some_and(animated_css);
    }
    let mut found = Vec::new();
    if texts {
        found.push("its <text> shows in browsers; native platforms draw the SVG without it");
    }
    if animation {
        found.push("its animation plays in browsers; native platforms draw the SVG still");
    }
    if images {
        found.push(
            "its <image> elements show in browsers; native platforms draw the SVG without them",
        );
    }
    found
}

/// `logo@2x.png` -> 2.0: the pixel density encoded in the file name.
fn density(path: &Path) -> f64 {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    match stem.rsplit_once('@').and_then(|(_, d)| d.strip_suffix('x')) {
        Some(d) => d.parse().unwrap_or(1.0),
        None => 1.0,
    }
}

impl NshPlugin {
    /// The entry's first statement, giving the renderer the app's themes.
    fn prologue(&self) -> Option<String> {
        let json = |v: &Option<String>| match v {
            Some(json) => serde_json::Value::String(json.clone()).to_string(),
            None => "null".to_owned(),
        };
        let setup = &self.tailwind;
        (setup.json.is_some() || setup.ui.is_some() || setup.appearance.is_some()).then(|| {
            format!(
                "_$setup({}, {}, {});",
                json(&setup.json),
                json(&setup.ui),
                json(&setup.appearance.map(str::to_owned))
            )
        })
    }

    /// Records `message` as the build's error (shown as it is).
    fn refuse(&self, message: String) -> anyhow::Error {
        self.error
            .lock()
            .expect("error lock")
            .get_or_insert_with(|| message.clone());
        anyhow!(message)
    }

    /// `path` relative to the app's directory with `/` separators, as asset
    /// URLs, module ids and messages name it on every system.
    fn rel(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn fail(&self, path: &Path, detail: &str) -> anyhow::Error {
        let message = format!("{}\n{detail}", self.rel(path));
        self.error
            .lock()
            .expect("error lock")
            .get_or_insert_with(|| message.clone());
        anyhow!(message)
    }
}

impl Plugin for NshPlugin {
    fn name(&self) -> Cow<'static, str> {
        "nsh".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
        let usage = HookUsage::ResolveId | HookUsage::Load | HookUsage::Transform;
        if self.split {
            usage | HookUsage::RenderChunk
        } else {
            usage
        }
    }

    async fn render_chunk(
        &self,
        _ctx: &PluginContext,
        args: &HookRenderChunkArgs<'_>,
    ) -> HookRenderChunkReturn {
        // What each chunk imports statically, by file name.
        let imports: std::collections::HashMap<&str, Vec<String>> = args
            .chunks
            .values()
            .map(|c| {
                let own = c.filename.as_str();
                let imports =
                    crate::bundle::chunk_imports(c.imports.iter().map(|i| i.as_str()), own);
                (own, imports)
            })
            .collect();
        let rewritten = crate::split::rewrite_imports(&args.code, &args.chunk.filename, |id| {
            imports.get(id).map(Vec::as_slice)
        });
        Ok(rewritten.map(|(code, map)| HookRenderChunkOutput {
            code,
            map: HookTransformOutputMap::Sourcemap(Box::new(map)),
        }))
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> HookResolveIdReturn {
        let spec = args.specifier;
        if self.modules.contains(spec) {
            self.natives
                .lock()
                .expect("natives lock")
                .insert(spec.to_owned());
            return Ok(Some(HookResolveIdOutput {
                external: Some(true.into()),
                ..HookResolveIdOutput::from_id(spec)
            }));
        }
        let from = args
            .importer
            .map(|i| format!("{}: ", self.rel(Path::new(i))))
            .unwrap_or_default();
        if spec == "solid-js" || spec.starts_with("solid-js/") {
            return Err(self.refuse(format!(
                "{from}`{spec}` is not part of nsh apps: import signal, computed, effect, batch, untracked, cleanup, context and mount from \"nsh\""
            )));
        }
        if spec.starts_with("nsh/") || spec.starts_with("nsh:") {
            return Err(self.refuse(format!("{from}nsh has no module `{spec}`")));
        }
        Ok(None)
    }

    async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
        let path = Path::new(args.id);
        let rel = self.rel(path);
        if has_extension(path, FONT_EXTENSIONS) {
            // A font import is its family name; the runtime registers the
            // font before the first frame.
            let data = std::fs::read(path).map_err(|e| self.fail(path, &e.to_string()))?;
            let family = font_family(&data)
                .map_err(|e| self.fail(path, &format!("cannot read font: {e}")))?;
            self.assets
                .lock()
                .expect("assets lock")
                .push(path.to_path_buf());
            let code = format!(
                "import {{ font }} from \"nsh:rt\";\nexport default font({uri:?}, {family:?});",
                uri = format!("asset:/{rel}"),
            );
            return Ok(Some(HookLoadOutput {
                code: code.into(),
                module_type: Some(ModuleType::Js),
                ..Default::default()
            }));
        }
        if !has_extension(path, IMAGE_EXTENSIONS) {
            return Ok(None);
        }
        // An image import is an asset descriptor; the natural size (in
        // points) lets the image lay out before it is decoded. An SVG's is
        // its own (a file name's `@2x` does not apply), as the native
        // backends read it.
        let (w, h) = if has_extension(path, &["svg"]) {
            let data = std::fs::read(path).map_err(|e| self.fail(path, &e.to_string()))?;
            let ((w, h), browser_only) =
                svg_asset(&data).map_err(|e| self.fail(path, &format!("cannot read SVG: {e}")))?;
            for difference in browser_only {
                log::warn!(target: "nsh::build", "{rel}: {difference}");
            }
            (w as f64, h as f64)
        } else {
            let dim = imagesize::size(path)
                .map_err(|e| self.fail(path, &format!("cannot read image: {e}")))?;
            let scale = density(path);
            (dim.width as f64 / scale, dim.height as f64 / scale)
        };
        self.assets
            .lock()
            .expect("assets lock")
            .push(path.to_path_buf());
        let code = format!(
            "export default {{ uri: {uri:?}, width: {w}, height: {h} }};",
            uri = format!("asset:/{rel}"),
        );
        Ok(Some(HookLoadOutput {
            code: code.into(),
            module_type: Some(ModuleType::Js),
            ..Default::default()
        }))
    }

    async fn transform(
        &self,
        _ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> HookTransformReturn {
        let jsx_module = matches!(args.module_type, ModuleType::Tsx | ModuleType::Jsx);
        // Rolldown's own helpers.
        if args.id.starts_with('\0') {
            return Ok(None);
        }
        let path = Path::new(args.id);
        let entry = path == self.entry;
        let script = matches!(args.module_type, ModuleType::Ts | ModuleType::Js);
        if !(jsx_module || entry || self.hot && script) {
            return Ok(None);
        }
        let hot_key = self.hot.then(|| self.rel(path));
        // By module type, not file name: asset modules (`logo.png`) are JS.
        let source_type = match args.module_type {
            ModuleType::Tsx => SourceType::tsx(),
            ModuleType::Jsx => SourceType::jsx(),
            ModuleType::Ts => SourceType::ts(),
            _ => SourceType::mjs(),
        };
        if jsx_module {
            let rel = self.rel(path);
            for u in crate::classes::check(args.code, source_type, &self.tailwind.theme) {
                log::warn!(target: "nsh::build", "{rel}:{}:{}: unknown class `{}`", u.line, u.column, u.class);
            }
        }
        let prologue = if entry { self.prologue() } else { None };
        let (code, map) = compile_native(
            path,
            source_type,
            args.code,
            prologue.as_deref(),
            hot_key.as_deref(),
            &self.modules,
        )
        .map_err(|detail| self.fail(path, &detail))?;
        Ok(Some(HookTransformOutput {
            code: Some(code),
            map: map.into(),
            module_type: Some(ModuleType::Js),
            ..Default::default()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg(body: &str) -> Vec<u8> {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">{body}</svg>"#)
            .into_bytes()
    }

    #[test]
    fn svg_import_warns_about_what_browsers_alone_draw() {
        let warnings = |body| svg_asset(&svg(body)).unwrap().1;
        assert!(warnings(r#"<rect width="5" height="5"/>"#).is_empty());
        let text = warnings("<text>NSH</text>");
        assert_eq!(text.len(), 1);
        assert!(text[0].contains("<text>"), "{text:?}");
        for animated in [
            r#"<rect width="5" height="5"><animate attributeName="x" to="5" dur="1s"/></rect>"#,
            r#"<rect width="5" height="5"><animateTransform attributeName="transform" type="rotate" to="90" dur="1s"/></rect>"#,
            r#"<set attributeName="fill" to="red"/>"#,
            "<style>@keyframes spin { to { transform: rotate(1turn) } }</style>",
            "<style><![CDATA[ rect { animation: spin 1s } ]]></style>",
            r#"<rect width="5" height="5" style="animation: spin 1s"/>"#,
        ] {
            let found = warnings(animated);
            assert_eq!(found.len(), 1, "{animated}");
            assert!(found[0].contains("animation"), "{animated}: {found:?}");
        }
        let images = warnings(r#"<image href="data:image/png;base64,AAAA" width="5" height="5"/>"#);
        assert_eq!(images.len(), 1);
        assert!(images[0].contains("<image>"), "{images:?}");
        assert_eq!(warnings("<text>a</text><set/><image/>").len(), 3);
    }

    #[test]
    fn svg_import_sizes_and_rejects() {
        let (size, _) = svg_asset(&svg("")).unwrap();
        assert_eq!(size, (10.0, 10.0));
        assert!(svg_asset(b"<svg><rect").is_err());
        assert!(svg_asset(b"\x89PNG\r\n\x1a\n").is_err());
    }
}
