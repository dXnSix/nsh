//! Android (API 33+): AGSL, written from naga's IR by NSH.
//!
//! AGSL is SkSL's runtime shaders: GLSL ES 1.00 with its Appendix A, `float2`-style types,
//! `uniform shader` inputs sampled with `eval` at pixel coordinates, and one entry point,
//! `half4 main(float2 coord)`. The checker (`profile.rs`) keeps a shader to what that can say, so
//! the writer maps the IR one to one, except:
//!
//! - loops: the IR's `loop` with a condition `if` and a `continuing` update is a `for` again;
//! - the uniforms: AGSL has no uniform blocks or struct uniforms, so each field is a `uniform` of
//!   its own (`nsh_u<k>_<name>`), and a value of the uniforms' struct is never a value: the writer
//!   drops it from parameters and calls and reads `u.field` as that field's uniform;
//! - what ES 1.00 lacks: `switch` (an `if` chain), integer `/` and `%`, `abs` / `min` / `max` /
//!   `clamp` / `sign` / `dot` of integers, `round`, `trunc`, the hyperbolic functions, `transpose`,
//!   `determinant`, array constructors and assignment (element by element) are written out.

use std::{collections::BTreeSet, fmt::Write as _};

use naga::{
    BinaryOperator, Block, Expression, Function, Handle, Literal, MathFunction, Module, ScalarKind,
    Statement, SwitchValue, Type, TypeInner, UnaryOperator, VectorSize,
    proc::{NameKey, Namer},
    valid::{FunctionInfo, ModuleInfo},
};

use crate::{
    Error, INPUTS, Input, Shader, UniformType, internal, layout,
    profile::{Analysis, Builtin, loop_shape, returns, root_local},
};

/// A uniform an AGSL program declares, and where its values are: `count` elements of type `ty`,
/// `stride` bytes apart, from `offset` in `buffer` (a matrix's columns as [`UniformType::columns`]
/// says). Set with `RuntimeShader.setIntUniform` when `ty` is an integer type, else
/// `setFloatUniform`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct AgslUniform {
    /// Its name in the source.
    pub name: String,
    /// The buffer its values are in.
    pub value: AgslValue,
    /// The type of an element.
    pub ty: UniformType,
    /// Its byte offset in the buffer.
    pub offset: u32,
    /// Elements (1 unless an array).
    pub count: u32,
    /// Bytes from one element to the next.
    pub stride: u32,
}

/// The buffer an [`AgslUniform`]'s values are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgslValue {
    /// `nsh` ([`layout`]'s `NSH_*`).
    #[default]
    Nsh,
    /// The frame ([`layout`]'s `FRAME_*`).
    Frame,
    /// The app's uniforms.
    Uniforms,
}

/// A shader as AGSL: the source of a `RuntimeShader`, its uniforms and its inputs.
///
/// The program draws at pixel coordinates from the view's top-left (a paint's shader over the
/// view's canvas, or a `RenderEffect` of the view's node); each input is a `uniform shader` whose
/// coordinates are its pixels (`layout::INPUT_PX` is its size in them).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct AgslProgram {
    /// The source.
    pub source: String,
    /// Its uniforms.
    pub uniforms: Vec<AgslUniform>,
    /// Its `uniform shader`s and the inputs they are.
    pub shaders: Vec<(String, Input)>,
}

/// AGSL's words, ES 1.00's reserved ones and the built-in functions, which the app's names must
/// not be.
const KEYWORDS: &[&str] = &[
    // GLSL ES 1.00 keywords and reserved words.
    "attribute",
    "const",
    "uniform",
    "varying",
    "break",
    "continue",
    "do",
    "for",
    "while",
    "if",
    "else",
    "in",
    "out",
    "inout",
    "float",
    "int",
    "void",
    "bool",
    "true",
    "false",
    "lowp",
    "mediump",
    "highp",
    "precision",
    "invariant",
    "discard",
    "return",
    "mat2",
    "mat3",
    "mat4",
    "vec2",
    "vec3",
    "vec4",
    "ivec2",
    "ivec3",
    "ivec4",
    "bvec2",
    "bvec3",
    "bvec4",
    "sampler2D",
    "samplerCube",
    "struct",
    "asm",
    "class",
    "union",
    "enum",
    "typedef",
    "template",
    "this",
    "packed",
    "goto",
    "switch",
    "default",
    "case",
    "inline",
    "noinline",
    "volatile",
    "public",
    "static",
    "extern",
    "external",
    "interface",
    "flat",
    "long",
    "short",
    "double",
    "half",
    "fixed",
    "unsigned",
    "superp",
    "input",
    "output",
    "hvec2",
    "hvec3",
    "hvec4",
    "dvec2",
    "dvec3",
    "dvec4",
    "fvec2",
    "fvec3",
    "fvec4",
    "sampler1D",
    "sampler3D",
    "sampler1DShadow",
    "sampler2DShadow",
    "sampler2DRect",
    "sampler3DRect",
    "sampler2DRectShadow",
    "sizeof",
    "cast",
    "namespace",
    "using",
    "main",
    // SkSL's types and words.
    "half2",
    "half3",
    "half4",
    "half2x2",
    "half3x3",
    "half4x4",
    "float2",
    "float3",
    "float4",
    "float2x2",
    "float3x3",
    "float4x4",
    "float2x3",
    "float2x4",
    "float3x2",
    "float3x4",
    "float4x2",
    "float4x3",
    "int2",
    "int3",
    "int4",
    "bool2",
    "bool3",
    "bool4",
    "short2",
    "short3",
    "short4",
    "ushort",
    "ushort2",
    "ushort3",
    "ushort4",
    "uint",
    "uint2",
    "uint3",
    "uint4",
    "byte",
    "ubyte",
    "mat2x2",
    "mat3x3",
    "mat4x4",
    "mat2x3",
    "mat2x4",
    "mat3x2",
    "mat3x4",
    "mat4x2",
    "mat4x3",
    "shader",
    "colorFilter",
    "blender",
    "layout",
    "sample",
    "eval",
    "readonly",
    "writeonly",
    "buffer",
    "workgroup",
    "es3",
    "sk_FragCoord",
    "sk_Caps",
    "sk_SampleCoord",
    "noperspective",
    "compute",
    "threadgroup",
    "pixel_local",
    // Built-in functions.
    "radians",
    "degrees",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "pow",
    "exp",
    "log",
    "exp2",
    "log2",
    "sqrt",
    "inversesqrt",
    "abs",
    "sign",
    "floor",
    "ceil",
    "fract",
    "mod",
    "min",
    "max",
    "clamp",
    "mix",
    "step",
    "smoothstep",
    "length",
    "distance",
    "dot",
    "cross",
    "normalize",
    "faceforward",
    "reflect",
    "refract",
    "matrixCompMult",
    "lessThan",
    "lessThanEqual",
    "greaterThan",
    "greaterThanEqual",
    "equal",
    "notEqual",
    "any",
    "all",
    "not",
    "texture2D",
    "saturate",
    "unpremul",
    "toLinearSrgb",
    "fromLinearSrgb",
    "sinh",
    "cosh",
    "tanh",
    "asinh",
    "acosh",
    "atanh",
    "round",
    "roundEven",
    "trunc",
    "determinant",
    "inverse",
    "transpose",
    "outerProduct",
    "modf",
    "isinf",
    "isnan",
    "floatBitsToInt",
    "intBitsToFloat",
    "packHalf2x16",
    "unpackHalf2x16",
];

static KEYWORD_SET: std::sync::LazyLock<naga::proc::KeywordSet> =
    std::sync::LazyLock::new(|| naga::proc::KeywordSet::from_iter(KEYWORDS));

/// Helpers the program needs, by name; written before the app's functions.
#[derive(Default)]
struct Helpers {
    written: BTreeSet<String>,
    out: String,
}

impl Helpers {
    fn need(&mut self, name: &str, body: impl FnOnce() -> String) {
        if self.written.insert(name.to_string()) {
            self.out.push_str(&body());
            self.out.push('\n');
        }
    }
}

struct Writer<'a> {
    module: &'a Module,
    /// The length of the app's source: what is declared before it is the app's.
    user_len: usize,
    info: &'a ModuleInfo,
    analysis: &'a Analysis,
    names: naga::FastHashMap<NameKey, String>,
    /// The uniforms' fields' uniform names, by field index.
    fields: Vec<String>,
    /// Image fields' slots, by field index.
    slots: Vec<Option<u8>>,
    helpers: Helpers,
}

/// The names of `nsh`'s fields, by index.
const NSH_FIELDS: [&str; 4] = ["nsh_size", "nsh_scale", "nsh_time", "nsh_progress"];

impl Shader {
    /// The shader in AGSL.
    pub fn to_agsl(&self) -> Result<AgslProgram> {
        let module = &self.module;
        let mut names = naga::FastHashMap::default();
        Namer::default().reset(
            module,
            &KEYWORD_SET,
            naga::proc::KeywordSet::empty(),
            naga::proc::CaseInsensitiveKeywordSet::empty(),
            &["nsh_", "gl_", "sk_"],
            &mut names,
        );
        let mut program = AgslProgram::default();
        let mut header = String::new();
        let nsh_types = [
            UniformType::Vec2,
            UniformType::F32,
            UniformType::F32,
            UniformType::F32,
        ];
        let nsh_offsets = [
            layout::NSH_VIEW_SIZE,
            layout::NSH_SCALE,
            layout::NSH_TIME,
            layout::NSH_PROGRESS,
        ];
        for ((name, ty), offset) in NSH_FIELDS.iter().zip(nsh_types).zip(nsh_offsets) {
            let _ = writeln!(header, "uniform {} {name};", uniform_type_name(ty));
            program.uniforms.push(AgslUniform {
                name: name.to_string(),
                value: AgslValue::Nsh,
                ty,
                offset,
                count: 1,
                stride: 0,
            });
        }

        // The uniforms' fields, each a uniform of its own.
        let mut fields = Vec::new();
        let mut slots = Vec::new();
        for (index, u) in self.reflection.uniforms.iter().enumerate() {
            let sanitized: String = u
                .name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect();
            let name = format!("nsh_u{index}_{sanitized}");
            let (ty, count) = match u.ty {
                UniformType::Image(slot) => {
                    slots.push(Some(slot));
                    (UniformType::Vec2, 1)
                }
                ty => {
                    slots.push(None);
                    (ty, u.count.unwrap_or(1))
                }
            };
            let suffix = u.count.map(|n| format!("[{n}]")).unwrap_or_default();
            let name = if matches!(u.ty, UniformType::Image(_)) {
                format!("{name}_size")
            } else {
                name
            };
            let _ = writeln!(header, "uniform {} {name}{suffix};", uniform_type_name(ty));
            program.uniforms.push(AgslUniform {
                name: name.clone(),
                value: AgslValue::Uniforms,
                ty,
                offset: u.offset,
                count,
                stride: u.stride,
            });
            fields.push(name);
        }

        // The inputs: a `uniform shader`, where it is and its size in its own pixels.
        for i in 0..INPUTS {
            if self.analysis.inputs & (1 << i) == 0 {
                continue;
            }
            let input = Input::from_index(i).expect("an input");
            let offset = layout::FRAME_INPUTS + layout::INPUT_STRIDE * i as u32;
            let _ = writeln!(
                header,
                "uniform shader nsh_input_{i};\nuniform float4 nsh_rect_{i};\nuniform float2 nsh_px_{i};"
            );
            program.shaders.push((format!("nsh_input_{i}"), input));
            for (name, ty, at) in [
                (
                    format!("nsh_rect_{i}"),
                    UniformType::Vec4,
                    layout::INPUT_RECT,
                ),
                (format!("nsh_px_{i}"), UniformType::Vec2, layout::INPUT_PX),
            ] {
                program.uniforms.push(AgslUniform {
                    name,
                    value: AgslValue::Frame,
                    ty,
                    offset: offset + at,
                    count: 1,
                    stride: 0,
                });
            }
            let _ = writeln!(
                header,
                "float4 nsh_sample_{i}(float2 p) {{\n    float2 uv = (p - nsh_rect_{i}.xy) / nsh_rect_{i}.zw;\n    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {{ return float4(0.0); }}\n    return float4(nsh_input_{i}.eval(uv * nsh_px_{i}));\n}}"
            );
        }

        let mut writer = Writer {
            module,
            user_len: self.source.len(),
            info: &self.info,
            analysis: &self.analysis,
            names,
            fields,
            slots,
            helpers: Helpers::default(),
        };
        let mut body = String::new();
        writer.structs(&mut body);
        for &f in &self.analysis.functions {
            writer.function(f, &mut body).map_err(internal)?;
        }
        let main = &writer.names[&NameKey::Function(self.analysis.main)];
        let _ = write!(
            body,
            "half4 main(float2 nsh_coord) {{\n    return half4({main}(nsh_coord / nsh_scale));\n}}\n"
        );

        program.source = format!("{header}\n{}{body}", writer.helpers.out);
        Ok(program)
    }
}

fn uniform_type_name(ty: UniformType) -> &'static str {
    match ty {
        UniformType::F32 => "float",
        UniformType::I32 => "int",
        UniformType::Vec2 | UniformType::Image(_) => "float2",
        UniformType::Vec3 => "float3",
        UniformType::Vec4 => "float4",
        UniformType::IVec2 => "int2",
        UniformType::IVec3 => "int3",
        UniformType::IVec4 => "int4",
        UniformType::Mat2 => "float2x2",
        UniformType::Mat3 => "float3x3",
        UniformType::Mat4 => "float4x4",
    }
}

fn scalar_name(kind: ScalarKind) -> &'static str {
    match kind {
        ScalarKind::Float => "float",
        ScalarKind::Sint => "int",
        _ => "bool",
    }
}

/// An inner type's name; arrays are their element's (`array_len` gives the `[n]`).
fn inner_name(
    module: &Module,
    names: &naga::FastHashMap<NameKey, String>,
    inner: &TypeInner,
) -> String {
    match *inner {
        TypeInner::Scalar(s) => scalar_name(s.kind).into(),
        TypeInner::Vector { size, scalar } => format!("{}{}", scalar_name(scalar.kind), size as u8),
        TypeInner::Matrix { columns, rows, .. } => format!("float{}x{}", columns as u8, rows as u8),
        TypeInner::Array { base, .. } => inner_name(module, names, &module.types[base].inner),
        TypeInner::Pointer { base, .. } => inner_name(module, names, &module.types[base].inner),
        TypeInner::ValuePointer {
            size: Some(size),
            scalar,
            ..
        } => format!("{}{}", scalar_name(scalar.kind), size as u8),
        TypeInner::ValuePointer {
            size: None, scalar, ..
        } => scalar_name(scalar.kind).into(),
        TypeInner::Struct { .. } => {
            let handle = module
                .types
                .iter()
                .find(|(_, t)| &t.inner == inner)
                .map(|(h, _)| h);
            handle
                .and_then(|h| names.get(&NameKey::Type(h)).cloned())
                .unwrap_or_else(|| "S".into())
        }
        _ => "float".into(),
    }
}

fn array_len(inner: &TypeInner) -> Option<u32> {
    match *inner {
        TypeInner::Array {
            size: naga::ArraySize::Constant(n),
            ..
        } => Some(n.get()),
        _ => None,
    }
}

/// How many references make naga's writers keep an expression in a variable.
fn bake_ref_count(e: &Expression) -> usize {
    match *e {
        Expression::Access { .. } | Expression::AccessIndex { .. } => usize::MAX,
        Expression::Load { .. } => 1,
        _ => 2,
    }
}

type Result<T> = std::result::Result<T, Error>;

/// One function being written.
struct FunctionCx<'a> {
    handle: Handle<Function>,
    function: &'a Function,
    info: &'a FunctionInfo,
    /// Expressions kept in variables, by handle, and their names.
    baked: naga::FastHashMap<Handle<Expression>, String>,
    /// Array constants declared as local arrays.
    constants: naga::FastHashMap<Handle<naga::Constant>, String>,
}

impl<'a> Writer<'a> {
    fn ty(&self, ty: Handle<Type>) -> String {
        inner_name(self.module, &self.names, &self.module.types[ty].inner)
    }

    /// `T name` or `T name[n]`.
    fn decl(&self, ty: Handle<Type>, name: &str) -> String {
        match array_len(&self.module.types[ty].inner) {
            Some(n) => format!("{} {name}[{n}]", self.ty(ty)),
            None => format!("{} {name}", self.ty(ty)),
        }
    }

    fn is_uniforms(&self, ty: Option<Handle<Type>>) -> bool {
        ty.is_some() && ty == self.analysis.uniforms
    }

    fn structs(&self, out: &mut String) {
        for (handle, ty) in self.module.types.iter() {
            let TypeInner::Struct { ref members, .. } = ty.inner else {
                continue;
            };
            let user = self
                .module
                .types
                .get_span(handle)
                .to_range()
                .is_some_and(|r| r.start < self.user_len);
            let prelude = matches!(
                ty.name.as_deref(),
                Some("Nsh" | "Image" | "NshInput" | "NshFrame")
            );
            if !user || prelude || Some(handle) == self.analysis.uniforms {
                continue;
            }
            let _ = writeln!(out, "struct {} {{", self.names[&NameKey::Type(handle)]);
            for (i, member) in members.iter().enumerate() {
                let name = &self.names[&NameKey::StructMember(handle, i as u32)];
                let _ = writeln!(out, "    {};", self.decl(member.ty, name));
            }
            out.push_str("};\n");
        }
    }

    fn function(&mut self, handle: Handle<Function>, out: &mut String) -> Result<()> {
        let module: &'a Module = self.module;
        let info: &'a ModuleInfo = self.info;
        let function = &module.functions[handle];
        let mut cx = FunctionCx {
            handle,
            function,
            info: &info[handle],
            baked: Default::default(),
            constants: Default::default(),
        };
        let result = match function.result {
            Some(ref r) => self.ty(r.ty),
            None => "void".into(),
        };
        let params: Vec<String> = function
            .arguments
            .iter()
            .enumerate()
            .filter(|(_, a)| !self.is_uniforms(Some(a.ty)))
            .map(|(i, a)| {
                self.decl(
                    a.ty,
                    &self.names[&NameKey::FunctionArgument(handle, i as u32)],
                )
            })
            .collect();
        let _ = writeln!(
            out,
            "{result} {}({}) {{",
            self.names[&NameKey::Function(handle)],
            params.join(", ")
        );

        // Array constants this function reads, as local arrays.
        for (_, e) in function.expressions.iter() {
            if let Expression::Constant(c) = *e
                && !cx.constants.contains_key(&c)
            {
                let constant = &self.module.constants[c];
                if let Some(n) = array_len(&self.module.types[constant.ty].inner) {
                    let name = format!("nsh_c{}", c.index());
                    let _ = writeln!(out, "    {};", self.decl(constant.ty, &name));
                    for i in 0..n {
                        let element = self.global_element(constant.init, i)?;
                        let _ = writeln!(out, "    {name}[{i}] = {element};");
                    }
                    cx.constants.insert(c, name);
                }
            }
        }

        // Array values that are constant expressions (no statement evaluates them), as local arrays:
        // ES 1.00 has no array constructors.
        let mut emitted = std::collections::HashSet::new();
        collect_emitted(&function.body, &mut emitted);
        for (h, e) in function.expressions.iter() {
            if !matches!(e, Expression::Compose { .. } | Expression::ZeroValue(_))
                || emitted.contains(&h)
                || cx.info[h].ref_count == 0
            {
                continue;
            }
            let Some(n) = array_len(self.inner(&cx, h)) else {
                continue;
            };
            let Some(ty) = self.value_type(&cx, h) else {
                continue;
            };
            let name = format!("nsh_a{}", h.index());
            let _ = writeln!(out, "    {};", self.decl(ty, &name));
            for i in 0..n {
                let element = self.element(&mut cx, h, i)?;
                let _ = writeln!(out, "    {name}[{i}] = {element};");
            }
            cx.baked.insert(h, name);
        }

        // Locals, zero unless initialized (WGSL's), but loop counters, which their `for` declares.
        for (v, var) in function.local_variables.iter() {
            if self.analysis.loops.contains_key(&(handle, v)) {
                continue;
            }
            let name = self.names[&NameKey::FunctionLocal(handle, v)].clone();
            match array_len(&self.module.types[var.ty].inner) {
                Some(n) => {
                    let _ = writeln!(out, "    {};", self.decl(var.ty, &name));
                    for i in 0..n {
                        let value = match var.init {
                            Some(init) => self.element(&mut cx, init, i)?,
                            None => self.zero(self.element_type(var.ty)),
                        };
                        let _ = writeln!(out, "    {name}[{i}] = {value};");
                    }
                }
                None => {
                    let value = match var.init {
                        Some(init) => self.expr(&mut cx, init)?,
                        None => self.zero(var.ty),
                    };
                    let _ = writeln!(out, "    {} = {value};", self.decl(var.ty, &name));
                }
            }
        }

        self.block(&mut cx, &function.body, 0, 1, out)?;
        if let Some(ref r) = function.result
            && !returns(&function.body)
        {
            let _ = writeln!(out, "    return {};", self.zero(r.ty));
        }
        out.push_str("}\n");
        Ok(())
    }

    fn element_type(&self, ty: Handle<Type>) -> Handle<Type> {
        match self.module.types[ty].inner {
            TypeInner::Array { base, .. } => base,
            _ => ty,
        }
    }

    fn zero(&self, ty: Handle<Type>) -> String {
        match self.module.types[ty].inner {
            TypeInner::Scalar(s) => match s.kind {
                ScalarKind::Float => "0.0".into(),
                ScalarKind::Sint => "0".into(),
                _ => "false".into(),
            },
            TypeInner::Vector { scalar, size } => {
                let v = match scalar.kind {
                    ScalarKind::Float => "0.0",
                    ScalarKind::Sint => "0",
                    _ => "false",
                };
                format!("{}{}({v})", scalar_name(scalar.kind), size as u8)
            }
            TypeInner::Matrix { columns, rows, .. } => {
                format!("float{}x{}(0.0)", columns as u8, rows as u8)
            }
            TypeInner::Struct { ref members, .. } => {
                let values: Vec<String> = members.iter().map(|m| self.zero(m.ty)).collect();
                format!("{}({})", self.ty(ty), values.join(", "))
            }
            _ => "0.0".into(),
        }
    }

    /// Element `i` of an array-valued expression.
    fn element(
        &mut self,
        cx: &mut FunctionCx<'a>,
        h: Handle<Expression>,
        i: u32,
    ) -> Result<String> {
        if let Some(name) = cx.baked.get(&h) {
            return Ok(format!("{name}[{i}]"));
        }
        match cx.function.expressions[h] {
            Expression::Compose { ref components, .. } => self.expr(cx, components[i as usize]),
            Expression::ZeroValue(ty) => Ok(self.zero(self.element_type(ty))),
            Expression::Constant(c) => self.global_element(self.module.constants[c].init, i),
            _ => Ok(format!("{}[{i}]", self.expr(cx, h)?)),
        }
    }

    /// Element `i` of an array-valued constant expression.
    fn global_element(&self, h: Handle<Expression>, i: u32) -> Result<String> {
        match self.module.global_expressions[h] {
            Expression::Compose { ref components, .. } => self.global(components[i as usize]),
            Expression::ZeroValue(ty) => Ok(self.zero(self.element_type(ty))),
            Expression::Constant(c) => self.global_element(self.module.constants[c].init, i),
            _ => Err(Error::new("an array constant")),
        }
    }

    /// A constant expression of the module.
    fn global(&self, h: Handle<Expression>) -> Result<String> {
        let m = self.module;
        Ok(match m.global_expressions[h] {
            Expression::Literal(l) => literal(l)?,
            Expression::Constant(c) => self.global(m.constants[c].init)?,
            Expression::ZeroValue(ty) => self.zero(ty),
            Expression::Compose { ty, ref components } => {
                let parts: Result<Vec<String>> =
                    components.iter().map(|&c| self.global(c)).collect();
                format!("{}({})", self.ty(ty), parts?.join(", "))
            }
            Expression::Splat { size, value } => {
                let v = self.global(value)?;
                let kind = match m.global_expressions[value] {
                    Expression::Literal(l) => l.scalar().kind,
                    _ => ScalarKind::Float,
                };
                format!("{}{}({v})", scalar_name(kind), size as u8)
            }
            _ => return Err(Error::new("a constant expression AGSL cannot write")),
        })
    }

    fn inner(&self, cx: &FunctionCx<'a>, h: Handle<Expression>) -> &'a TypeInner {
        let module: &'a Module = self.module;
        let info: &'a FunctionInfo = cx.info;
        info[h].ty.inner_with(&module.types)
    }

    fn value_type(&self, cx: &FunctionCx<'a>, h: Handle<Expression>) -> Option<Handle<Type>> {
        match cx.info[h].ty {
            naga::proc::TypeResolution::Handle(ty) => Some(ty),
            _ => None,
        }
    }

    /// The type's name of an expression's value.
    fn expr_type(&self, cx: &FunctionCx<'a>, h: Handle<Expression>) -> String {
        inner_name(self.module, &self.names, self.inner(cx, h))
    }

    fn block(
        &mut self,
        cx: &mut FunctionCx<'a>,
        block: &Block,
        skip: usize,
        level: usize,
        out: &mut String,
    ) -> Result<()> {
        for stmt in block.iter().skip(skip) {
            self.statement(cx, stmt, level, out)?;
        }
        Ok(())
    }

    fn statement(
        &mut self,
        cx: &mut FunctionCx<'a>,
        stmt: &Statement,
        level: usize,
        out: &mut String,
    ) -> Result<()> {
        let pad = "    ".repeat(level);
        match *stmt {
            Statement::Emit(ref range) => {
                for h in range.clone() {
                    self.emit(cx, h, &pad, out)?;
                }
            }
            Statement::Block(ref b) => {
                let _ = writeln!(out, "{pad}{{");
                self.block(cx, b, 0, level + 1, out)?;
                let _ = writeln!(out, "{pad}}}");
            }
            Statement::If {
                condition,
                ref accept,
                ref reject,
            } => {
                let c = self.expr(cx, condition)?;
                let _ = writeln!(out, "{pad}if ({c}) {{");
                self.block(cx, accept, 0, level + 1, out)?;
                if !reject.is_empty() {
                    let _ = writeln!(out, "{pad}}} else {{");
                    self.block(cx, reject, 0, level + 1, out)?;
                }
                let _ = writeln!(out, "{pad}}}");
            }
            Statement::Switch {
                selector,
                ref cases,
            } => {
                let name = format!("nsh_selector{}", selector.index());
                let s = self.expr(cx, selector)?;
                let _ = writeln!(out, "{pad}int {name} = {s};");
                // Cases that fall through share the body of the case they fall into; the default
                // case is the last `else` whatever values share it.
                let mut groups: Vec<(Vec<i32>, bool, &Block)> = Vec::new();
                let mut values = Vec::new();
                let mut default = false;
                for case in cases {
                    match case.value {
                        SwitchValue::I32(v) => values.push(v),
                        SwitchValue::U32(v) => values.push(v as i32),
                        SwitchValue::Default => default = true,
                    }
                    if !case.fall_through {
                        groups.push((
                            std::mem::take(&mut values),
                            std::mem::take(&mut default),
                            &case.body,
                        ));
                    }
                }
                let mut first = true;
                for (values, _, body) in groups.iter().filter(|g| !g.1) {
                    let test: Vec<String> =
                        values.iter().map(|v| format!("{name} == {v}")).collect();
                    let _ = writeln!(
                        out,
                        "{pad}{}if ({}) {{",
                        if first { "" } else { "} else " },
                        test.join(" || ")
                    );
                    self.block(cx, body, 0, level + 1, out)?;
                    first = false;
                }
                if let Some((_, _, body)) = groups.iter().find(|g| g.1) {
                    if first {
                        let _ = writeln!(out, "{pad}{{");
                    } else {
                        let _ = writeln!(out, "{pad}}} else {{");
                    }
                    self.block(cx, body, 0, level + 1, out)?;
                    first = false;
                }
                if !first {
                    let _ = writeln!(out, "{pad}}}");
                }
            }
            Statement::Loop {
                ref body,
                ref continuing,
                ..
            } => {
                let shape = loop_shape(cx.function, body, continuing)
                    .ok_or_else(|| Error::new("a loop"))?;
                let l = self.analysis.loops[&(cx.handle, shape.counter)];
                let name = self.names[&NameKey::FunctionLocal(cx.handle, shape.counter)].clone();
                let int = l.kind == ScalarKind::Sint;
                let num = |v: f64| {
                    if int {
                        format!("{}", v as i64)
                    } else {
                        float(v as f32)
                    }
                };
                let op = naga::back::binary_operation_str(l.op);
                let step = match (int, l.step) {
                    (true, 1.0) => format!("{name}++"),
                    (true, -1.0) => format!("{name}--"),
                    (_, s) if s < 0.0 => format!("{name} -= {}", num(-s)),
                    (_, s) => format!("{name} += {}", num(s)),
                };
                let _ = writeln!(
                    out,
                    "{pad}for ({} {name} = {}; {name} {op} {}; {step}) {{",
                    if int { "int" } else { "float" },
                    num(l.start),
                    num(l.bound)
                );
                self.block(cx, body, l.skip, level + 1, out)?;
                let _ = writeln!(out, "{pad}}}");
            }
            Statement::Break => {
                let _ = writeln!(out, "{pad}break;");
            }
            Statement::Continue => {
                let _ = writeln!(out, "{pad}continue;");
            }
            Statement::Return { value } => match value {
                Some(v) => {
                    let v = self.expr(cx, v)?;
                    let _ = writeln!(out, "{pad}return {v};");
                }
                None => {
                    let _ = writeln!(out, "{pad}return;");
                }
            },
            Statement::Store { pointer, value } => {
                if let Some(v) = root_local(cx.function, pointer)
                    && self.analysis.loops.contains_key(&(cx.handle, v))
                {
                    // A counter's start: its `for` sets it.
                    return Ok(());
                }
                let target = self.expr(cx, pointer)?;
                let len = match *self.inner(cx, pointer) {
                    TypeInner::Pointer { base, .. } => array_len(&self.module.types[base].inner),
                    _ => None,
                };
                match len {
                    Some(n) => {
                        for i in 0..n {
                            let e = self.element(cx, value, i)?;
                            let _ = writeln!(out, "{pad}{target}[{i}] = {e};");
                        }
                    }
                    None => {
                        let v = self.expr(cx, value)?;
                        let _ = writeln!(out, "{pad}{target} = {v};");
                    }
                }
            }
            Statement::Call {
                function: callee,
                ref arguments,
                result,
            } => {
                let call = match self.analysis.builtins.get(&callee) {
                    Some(&Builtin::Sample(input)) => format!(
                        "nsh_sample_{}({})",
                        input.index(),
                        self.expr(cx, arguments[0])?
                    ),
                    Some(Builtin::SampleImage) => {
                        let slot = match cx.function.expressions[arguments[0]] {
                            Expression::AccessIndex { index, .. } => {
                                self.slots.get(index as usize).copied().flatten()
                            }
                            _ => None,
                        }
                        .ok_or_else(|| Error::new("an image"))?;
                        format!(
                            "nsh_sample_{}({})",
                            Input::Image(slot).index(),
                            self.expr(cx, arguments[1])?
                        )
                    }
                    None => {
                        let mut args = Vec::new();
                        for &a in arguments {
                            if !self.is_uniforms(self.value_type(cx, a)) {
                                args.push(self.expr(cx, a)?);
                            }
                        }
                        format!(
                            "{}({})",
                            self.names[&NameKey::Function(callee)],
                            args.join(", ")
                        )
                    }
                };
                match result {
                    Some(r) => {
                        let name = self.bake_name(cx, r);
                        let ty = self.expr_type(cx, r);
                        let _ = writeln!(out, "{pad}{ty} {name} = {call};");
                        cx.baked.insert(r, name);
                    }
                    None => {
                        let _ = writeln!(out, "{pad}{call};");
                    }
                }
            }
            _ => return Err(Error::new("a statement AGSL cannot write")),
        }
        Ok(())
    }

    fn bake_name(&self, cx: &FunctionCx<'a>, h: Handle<Expression>) -> String {
        match cx.function.named_expressions.get(&h) {
            Some(name) => format!("{}_n{}", sanitize(name), h.index()),
            None => format!("nsh_e{}", h.index()),
        }
    }

    /// An expression where its statement evaluates it: kept in a variable when naga's writers
    /// would keep it (named, loaded, used twice), or when it is an array.
    fn emit(
        &mut self,
        cx: &mut FunctionCx<'a>,
        h: Handle<Expression>,
        pad: &str,
        out: &mut String,
    ) -> Result<()> {
        let e = &cx.function.expressions[h];
        let inner = self.inner(cx, h);
        if matches!(
            inner,
            TypeInner::Pointer { .. } | TypeInner::ValuePointer { .. }
        ) {
            return Ok(());
        }
        let ty = self.value_type(cx, h);
        if self.is_uniforms(ty) || (ty.is_some() && ty == self.analysis.image) {
            // Never values in AGSL: read where they are used.
            return Ok(());
        }
        if self.index_like(cx, h) {
            // ES 1.00 indexes with loop counters and constants only, not variables holding them;
            // the body leaves counters alone, so reading them where they are used is the same.
            return Ok(());
        }
        let array = array_len(inner);
        let named = cx.function.named_expressions.contains_key(&h);
        let bake = named || array.is_some() || cx.info[h].ref_count >= bake_ref_count(e);
        if !bake {
            return Ok(());
        }
        if let Some(n) = array {
            // Uniform arrays read in place are not copied.
            if !named && matches!(e, Expression::AccessIndex { .. }) {
                return Ok(());
            }
            let name = self.bake_name(cx, h);
            let ty = ty.ok_or_else(|| Error::new("an array type"))?;
            let _ = writeln!(out, "{pad}{};", self.decl(ty, &name));
            for i in 0..n {
                let v = self.element(cx, h, i)?;
                let _ = writeln!(out, "{pad}{name}[{i}] = {v};");
            }
            cx.baked.insert(h, name);
            return Ok(());
        }
        let value = self.expr(cx, h)?;
        let name = self.bake_name(cx, h);
        let _ = writeln!(out, "{pad}{} {name} = {value};", self.expr_type(cx, h));
        cx.baked.insert(h, name);
        Ok(())
    }

    /// Whether an expression is made of constants and loop counters only.
    fn index_like(&self, cx: &FunctionCx<'a>, h: Handle<Expression>) -> bool {
        match cx.function.expressions[h] {
            Expression::Literal(_) | Expression::Constant(_) => true,
            Expression::Load { pointer } => {
                matches!(cx.function.expressions[pointer], Expression::LocalVariable(v) if self.analysis.loops.contains_key(&(cx.handle, v)))
            }
            Expression::Binary {
                op: BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply,
                left,
                right,
            } => self.index_like(cx, left) && self.index_like(cx, right),
            Expression::Unary {
                op: UnaryOperator::Negate,
                expr,
            } => self.index_like(cx, expr),
            _ => false,
        }
    }

    fn expr(&mut self, cx: &mut FunctionCx<'a>, h: Handle<Expression>) -> Result<String> {
        if let Some(name) = cx.baked.get(&h) {
            return Ok(name.clone());
        }
        let m = self.module;
        let e = &cx.function.expressions[h];
        Ok(match *e {
            Expression::Literal(l) => literal(l)?,
            Expression::Constant(c) => match cx.constants.get(&c) {
                Some(name) => name.clone(),
                None => self.global(m.constants[c].init)?,
            },
            Expression::ZeroValue(ty) => self.zero(ty),
            Expression::Compose { ty, ref components } => {
                let mut parts = Vec::new();
                for &c in components {
                    parts.push(self.expr(cx, c)?);
                }
                format!("{}({})", self.ty(ty), parts.join(", "))
            }
            Expression::Access { base, index } => {
                format!("{}[{}]", self.expr(cx, base)?, self.expr(cx, index)?)
            }
            Expression::AccessIndex { base, index } => self.access_index(cx, base, index)?,
            Expression::Splat { size, value } => {
                let kind = self
                    .inner(cx, value)
                    .scalar_kind()
                    .unwrap_or(ScalarKind::Float);
                format!(
                    "{}{}({})",
                    scalar_name(kind),
                    size as u8,
                    self.expr(cx, value)?
                )
            }
            Expression::Swizzle {
                size,
                vector,
                pattern,
            } => {
                let v = self.expr(cx, vector)?;
                let p: String = pattern[..size as usize]
                    .iter()
                    .map(|c| ['x', 'y', 'z', 'w'][*c as usize])
                    .collect();
                format!("{v}.{p}")
            }
            Expression::FunctionArgument(i) => {
                self.names[&NameKey::FunctionArgument(cx.handle, i)].clone()
            }
            Expression::LocalVariable(v) => {
                self.names[&NameKey::FunctionLocal(cx.handle, v)].clone()
            }
            Expression::Load { pointer } => self.expr(cx, pointer)?,
            Expression::GlobalVariable(_) => return Err(Error::new("a global read whole")),
            Expression::Unary { op, expr } => {
                let x = self.expr(cx, expr)?;
                let vector = matches!(self.inner(cx, expr), TypeInner::Vector { .. });
                match op {
                    UnaryOperator::Negate => format!("(-{x})"),
                    UnaryOperator::LogicalNot if vector => format!("not({x})"),
                    UnaryOperator::LogicalNot | UnaryOperator::BitwiseNot => format!("(!{x})"),
                }
            }
            Expression::Binary { op, left, right } => self.binary(cx, op, left, right)?,
            Expression::Select {
                condition,
                accept,
                reject,
            } => {
                let c = self.expr(cx, condition)?;
                let a = self.expr(cx, accept)?;
                let r = self.expr(cx, reject)?;
                match *self.inner(cx, condition) {
                    TypeInner::Vector { size, .. } => {
                        let ty = self.expr_type(cx, accept);
                        let parts: Vec<String> = (0..size as usize)
                            .map(|i| format!("({c}[{i}] ? {a}[{i}] : {r}[{i}])"))
                            .collect();
                        format!("{ty}({})", parts.join(", "))
                    }
                    _ => format!("({c} ? {a} : {r})"),
                }
            }
            Expression::Relational { fun, argument } => {
                let x = self.expr(cx, argument)?;
                match (fun, self.inner(cx, argument)) {
                    (_, TypeInner::Scalar(_)) => x,
                    (naga::RelationalFunction::All, _) => format!("all({x})"),
                    _ => format!("any({x})"),
                }
            }
            Expression::Math {
                fun,
                arg,
                arg1,
                arg2,
                arg3: _,
            } => self.math(cx, fun, arg, arg1, arg2)?,
            Expression::As { expr, kind, .. } => {
                let x = self.expr(cx, expr)?;
                match *self.inner(cx, expr) {
                    TypeInner::Vector { size, .. } => {
                        format!("{}{}({x})", scalar_name(kind), size as u8)
                    }
                    _ => format!("{}({x})", scalar_name(kind)),
                }
            }
            Expression::CallResult(_) => return Err(Error::new("a call's result before the call")),
            _ => return Err(Error::new("an expression AGSL cannot write")),
        })
    }

    fn access_index(
        &mut self,
        cx: &mut FunctionCx<'a>,
        base: Handle<Expression>,
        index: u32,
    ) -> Result<String> {
        let m = self.module;
        // `nsh.field`
        if let Expression::GlobalVariable(g) = cx.function.expressions[base]
            && Some(g) == self.analysis.nsh
        {
            return Ok(NSH_FIELDS[index as usize].into());
        }
        let base_ty = self.value_type(cx, base);
        // `u.field`
        if self.is_uniforms(base_ty) {
            return Ok(self.fields[index as usize].clone());
        }
        // `u.image.size`
        if base_ty.is_some() && base_ty == self.analysis.image {
            if let Expression::AccessIndex { index: field, .. } = cx.function.expressions[base] {
                return Ok(self.fields[field as usize].clone());
            }
            return Err(Error::new("an image read where it is not a uniform"));
        }
        let b = self.expr(cx, base)?;
        let inner = match *self.inner(cx, base) {
            TypeInner::Pointer { base, .. } => &m.types[base].inner,
            ref inner => inner,
        };
        Ok(match *inner {
            TypeInner::Struct { .. } => {
                let ty = match cx.info[base].ty {
                    naga::proc::TypeResolution::Handle(ty) => ty,
                    naga::proc::TypeResolution::Value(TypeInner::Pointer { base, .. }) => base,
                    _ => return Err(Error::new("a struct's type")),
                };
                format!("{b}.{}", self.names[&NameKey::StructMember(ty, index)])
            }
            TypeInner::Vector { .. } | TypeInner::ValuePointer { size: Some(_), .. } => {
                format!("{b}.{}", ['x', 'y', 'z', 'w'][index as usize])
            }
            _ => format!("{b}[{index}]"),
        })
    }

    fn binary(
        &mut self,
        cx: &mut FunctionCx<'a>,
        op: BinaryOperator,
        left: Handle<Expression>,
        right: Handle<Expression>,
    ) -> Result<String> {
        let a = self.expr(cx, left)?;
        let b = self.expr(cx, right)?;
        let (lt, rt) = (self.inner(cx, left).clone(), self.inner(cx, right).clone());
        let kind = lt.scalar_kind().unwrap_or(ScalarKind::Float);
        let vector = |t: &TypeInner| match *t {
            TypeInner::Vector { size, .. } => Some(size),
            _ => None,
        };
        let size = vector(&lt).or(vector(&rt));
        let tname = |t: &TypeInner| inner_name(self.module, &self.names, t);
        use BinaryOperator as Bo;
        Ok(match op {
            Bo::Add | Bo::Subtract | Bo::Multiply => {
                format!("({a} {} {b})", naga::back::binary_operation_str(op))
            }
            Bo::Modulo | Bo::Divide if op == Bo::Modulo || kind == ScalarKind::Sint => {
                // Helpers over one type: a scalar beside a vector is splat.
                let result = tname(if vector(&lt).is_some() { &lt } else { &rt });
                let splat = |x: &str, t: &TypeInner| {
                    if size.is_some() && vector(t).is_none() {
                        format!("{result}({x})")
                    } else {
                        x.to_string()
                    }
                };
                let (a, b) = (splat(&a, &lt), splat(&b, &rt));
                let (base, body) = match (op, kind) {
                    // WGSL: `x / 0` is `x`, `x % 0` is 0.
                    (Bo::Divide, _) => (
                        "idiv",
                        format!(
                            "return {};",
                            int_componentwise(&result, size, "a", "b", |x, y| format!(
                                "({y} == 0 ? {x} : {x} / {y})"
                            ))
                        ),
                    ),
                    (_, ScalarKind::Sint) => (
                        "imod",
                        format!(
                            "return {};",
                            int_componentwise(&result, size, "a", "b", |x, y| format!(
                                "({y} == 0 ? 0 : {x} - ({x} / {y}) * {y})"
                            ))
                        ),
                    ),
                    // WGSL's `%` truncates; ES 1.00's `mod` floors.
                    _ => (
                        "fmod",
                        format!("{result} q = a / b; return a - b * (sign(q) * floor(abs(q)));"),
                    ),
                };
                let name = format!("nsh_{base}_{result}");
                self.helpers.need(&name, || {
                    format!("{result} {name}({result} a, {result} b) {{ {body} }}")
                });
                format!("{name}({a}, {b})")
            }
            Bo::Divide | Bo::Modulo => format!("({a} / {b})"),
            Bo::Equal
            | Bo::NotEqual
            | Bo::Less
            | Bo::LessEqual
            | Bo::Greater
            | Bo::GreaterEqual
                if size.is_some() =>
            {
                let fun = match op {
                    Bo::Equal => "equal",
                    Bo::NotEqual => "notEqual",
                    Bo::Less => "lessThan",
                    Bo::LessEqual => "lessThanEqual",
                    Bo::Greater => "greaterThan",
                    _ => "greaterThanEqual",
                };
                format!("{fun}({a}, {b})")
            }
            Bo::Equal
            | Bo::NotEqual
            | Bo::Less
            | Bo::LessEqual
            | Bo::Greater
            | Bo::GreaterEqual => {
                format!("({a} {} {b})", naga::back::binary_operation_str(op))
            }
            Bo::And | Bo::LogicalAnd | Bo::InclusiveOr | Bo::LogicalOr | Bo::ExclusiveOr => {
                let o = match op {
                    Bo::And | Bo::LogicalAnd => "&&",
                    Bo::InclusiveOr | Bo::LogicalOr => "||",
                    _ => "!=",
                };
                match size {
                    Some(size) if o == "!=" => {
                        let _ = size;
                        format!("notEqual({a}, {b})")
                    }
                    Some(size) => {
                        let parts: Vec<String> = (0..size as usize)
                            .map(|i| format!("({a}[{i}] {o} {b}[{i}])"))
                            .collect();
                        format!("bool{}({})", size as u8, parts.join(", "))
                    }
                    None => format!("({a} {o} {b})"),
                }
            }
            Bo::ShiftLeft | Bo::ShiftRight => return Err(Error::new("a shift")),
        })
    }

    fn math(
        &mut self,
        cx: &mut FunctionCx<'a>,
        fun: MathFunction,
        arg: Handle<Expression>,
        arg1: Option<Handle<Expression>>,
        arg2: Option<Handle<Expression>>,
    ) -> Result<String> {
        use MathFunction as Mf;
        let x = self.expr(cx, arg)?;
        let y = arg1.map(|a| self.expr(cx, a)).transpose()?;
        let z = arg2.map(|a| self.expr(cx, a)).transpose()?;
        let inner = self.inner(cx, arg).clone();
        let t = inner_name(self.module, &self.names, &inner);
        let int = inner.scalar_kind() == Some(ScalarKind::Sint);
        let size = match inner {
            TypeInner::Vector { size, .. } => Some(size),
            _ => None,
        };
        let args = |list: &[&Option<String>]| -> String {
            let mut parts = vec![x.clone()];
            parts.extend(list.iter().filter_map(|s| (*s).clone()));
            parts.join(", ")
        };
        let call = |name: &str| format!("{name}({})", args(&[&y, &z]));
        // A helper over `x` (and `y`, `z`, of the same type) written once per type.
        let helper = |this: &mut Self, base: &str, params: usize, body: &str| -> String {
            let name = format!("nsh_{base}_{t}");
            let decl: Vec<String> = ["x", "y", "z"][..params]
                .iter()
                .map(|p| format!("{t} {p}"))
                .collect();
            this.helpers.need(&name, || {
                format!("{t} {name}({}) {{ {body} }}", decl.join(", "))
            });
            format!("{name}({})", args(&[&y, &z]))
        };
        Ok(match fun {
            Mf::Abs if int => {
                let body = format!(
                    "return {};",
                    int_componentwise(&t, size, "x", "x", |a, _| format!("({a} < 0 ? -{a} : {a})"))
                );
                helper(self, "abs", 1, &body)
            }
            Mf::Min if int => {
                let body = format!(
                    "return {};",
                    int_componentwise(&t, size, "x", "y", |a, b| format!(
                        "({a} < {b} ? {a} : {b})"
                    ))
                );
                helper(self, "min", 2, &body)
            }
            Mf::Max if int => {
                let body = format!(
                    "return {};",
                    int_componentwise(&t, size, "x", "y", |a, b| format!(
                        "({a} > {b} ? {a} : {b})"
                    ))
                );
                helper(self, "max", 2, &body)
            }
            Mf::Clamp if int => {
                let body = int_componentwise(&t, size, "x", "y", |a, b| {
                    format!("({a} < {b} ? {b} : {a})")
                });
                let body = format!(
                    "{t} m = {body}; return {};",
                    int_componentwise(&t, size, "m", "z", |a, b| format!(
                        "({a} > {b} ? {b} : {a})"
                    ))
                );
                helper(self, "clamp", 3, &body)
            }
            Mf::Sign if int => {
                let body = format!(
                    "return {};",
                    int_componentwise(&t, size, "x", "x", |a, _| format!(
                        "({a} > 0 ? 1 : ({a} < 0 ? -1 : 0))"
                    ))
                );
                helper(self, "sign", 1, &body)
            }
            Mf::Dot if int => {
                let n = size.map_or(1, |s| s as usize);
                let terms: Vec<String> = (0..n).map(|i| format!("x[{i}] * y[{i}]")).collect();
                let name = format!("nsh_dot_{t}");
                let body = terms.join(" + ");
                self.helpers.need(&name, || {
                    format!("int {name}({t} x, {t} y) {{ return {body}; }}")
                });
                format!("{name}({x}, {})", y.clone().unwrap_or_default())
            }
            Mf::Abs => call("abs"),
            Mf::Min => call("min"),
            Mf::Max => call("max"),
            Mf::Clamp => call("clamp"),
            Mf::Saturate => format!("clamp({x}, 0.0, 1.0)"),
            Mf::Cos => call("cos"),
            Mf::Sin => call("sin"),
            Mf::Tan => call("tan"),
            Mf::Acos => call("acos"),
            Mf::Asin => call("asin"),
            Mf::Atan | Mf::Atan2 => call("atan"),
            Mf::Cosh => helper(self, "cosh", 1, "return (exp(x) + exp(-x)) * 0.5;"),
            Mf::Sinh => helper(self, "sinh", 1, "return (exp(x) - exp(-x)) * 0.5;"),
            Mf::Tanh => helper(
                self,
                "tanh",
                1,
                &format!("{t} e = exp(-2.0 * abs(x)); return sign(x) * (1.0 - e) / (1.0 + e);"),
            ),
            Mf::Asinh => helper(self, "asinh", 1, "return log(x + sqrt(x * x + 1.0));"),
            Mf::Acosh => helper(self, "acosh", 1, "return log(x + sqrt(x * x - 1.0));"),
            Mf::Atanh => helper(self, "atanh", 1, "return 0.5 * log((1.0 + x) / (1.0 - x));"),
            Mf::Radians => call("radians"),
            Mf::Degrees => call("degrees"),
            Mf::Ceil => call("ceil"),
            Mf::Floor => call("floor"),
            Mf::Fract => call("fract"),
            Mf::Trunc => helper(self, "trunc", 1, "return sign(x) * floor(abs(x));"),
            Mf::Round => {
                // WGSL rounds halves to even.
                let tie = match size {
                    Some(n) => format!("float{}(equal(r - x, float{}(0.5)))", n as u8, n as u8),
                    None => "(r - x == 0.5 ? 1.0 : 0.0)".into(),
                };
                helper(
                    self,
                    "round",
                    1,
                    &format!("{t} r = floor(x + 0.5); return r - {tie} * mod(r, 2.0);"),
                )
            }
            Mf::Exp => call("exp"),
            Mf::Exp2 => call("exp2"),
            Mf::Log => call("log"),
            Mf::Log2 => call("log2"),
            Mf::Pow => call("pow"),
            Mf::Sqrt => call("sqrt"),
            Mf::InverseSqrt => call("inversesqrt"),
            Mf::Dot => call("dot"),
            Mf::Cross => call("cross"),
            Mf::Distance => call("distance"),
            Mf::Length => call("length"),
            Mf::Normalize => call("normalize"),
            Mf::FaceForward => call("faceforward"),
            Mf::Reflect => call("reflect"),
            Mf::Refract => call("refract"),
            Mf::Sign => call("sign"),
            Mf::Fma => format!(
                "({x} * {} + {})",
                y.clone().unwrap_or_default(),
                z.clone().unwrap_or_default()
            ),
            Mf::Mix => call("mix"),
            Mf::Step => call("step"),
            Mf::SmoothStep => call("smoothstep"),
            Mf::Transpose => {
                let n = match inner {
                    TypeInner::Matrix { columns, .. } => columns as usize,
                    _ => 2,
                };
                let parts: Vec<String> = (0..n)
                    .flat_map(|c| (0..n).map(move |r| format!("x[{r}][{c}]")))
                    .collect();
                helper(
                    self,
                    "transpose",
                    1,
                    &format!("return {t}({});", parts.join(", ")),
                )
            }
            Mf::Determinant => {
                let n = match inner {
                    TypeInner::Matrix { columns, .. } => columns as usize,
                    _ => 2,
                };
                let name = format!("nsh_determinant_{t}");
                let body = determinant(n);
                self.helpers
                    .need(&name, || format!("float {name}({t} m) {{ {body} }}"));
                format!("{name}({x})")
            }
            _ => return Err(Error::new("a function AGSL cannot write")),
        })
    }
}

/// `op(a, b)` component by component for an integer type of `size` components (a scalar: once).
fn int_componentwise(
    ty: &str,
    size: Option<VectorSize>,
    a: &str,
    b: &str,
    op: impl Fn(&str, &str) -> String,
) -> String {
    match size {
        None => op(a, b),
        Some(size) => {
            // `b` may be a scalar where `a` is a vector (`v / s`): index only what is a vector.
            let parts: Vec<String> = (0..size as usize)
                .map(|i| op(&format!("{a}[{i}]"), &format!("{b}[{i}]")))
                .collect();
            format!("{ty}({})", parts.join(", "))
        }
    }
}

fn determinant(n: usize) -> String {
    match n {
        2 => "return m[0][0] * m[1][1] - m[1][0] * m[0][1];".into(),
        3 => "return m[0][0] * (m[1][1] * m[2][2] - m[2][1] * m[1][2]) - m[1][0] * (m[0][1] * m[2][2] - m[2][1] * m[0][2]) + m[2][0] * (m[0][1] * m[1][2] - m[1][1] * m[0][2]);".into(),
        _ => "float b00 = m[0][0] * m[1][1] - m[0][1] * m[1][0]; float b01 = m[0][0] * m[1][2] - m[0][2] * m[1][0]; float b02 = m[0][0] * m[1][3] - m[0][3] * m[1][0]; float b03 = m[0][1] * m[1][2] - m[0][2] * m[1][1]; float b04 = m[0][1] * m[1][3] - m[0][3] * m[1][1]; float b05 = m[0][2] * m[1][3] - m[0][3] * m[1][2]; float b06 = m[2][0] * m[3][1] - m[2][1] * m[3][0]; float b07 = m[2][0] * m[3][2] - m[2][2] * m[3][0]; float b08 = m[2][0] * m[3][3] - m[2][3] * m[3][0]; float b09 = m[2][1] * m[3][2] - m[2][2] * m[3][1]; float b10 = m[2][1] * m[3][3] - m[2][3] * m[3][1]; float b11 = m[2][2] * m[3][3] - m[2][3] * m[3][2]; return b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;".into(),
    }
}

fn float(v: f32) -> String {
    let s = format!("{v:?}");
    let s = if s.contains(['.', 'e', 'E']) {
        s
    } else {
        format!("{s}.0")
    };
    if v.is_sign_negative() {
        format!("({s})")
    } else {
        s
    }
}

fn literal(l: Literal) -> Result<String> {
    Ok(match l {
        Literal::F32(v) => float(v),
        Literal::AbstractFloat(v) => float(v as f32),
        Literal::I32(v) if v == i32::MIN => "(-2147483647 - 1)".into(),
        Literal::I32(v) if v < 0 => format!("({v})"),
        Literal::I32(v) => v.to_string(),
        Literal::AbstractInt(v) if v < 0 => format!("({v})"),
        Literal::AbstractInt(v) => v.to_string(),
        Literal::Bool(v) => v.to_string(),
        _ => return Err(Error::new("a literal AGSL cannot write")),
    })
}

/// A name the app gave a `let`, safe in AGSL (the handle's index makes it unique).
fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let s = s.trim_matches('_').replace("__", "_");
    if s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("v{s}")
    } else {
        s
    }
}

fn collect_emitted(block: &Block, out: &mut std::collections::HashSet<Handle<Expression>>) {
    for stmt in block.iter() {
        match *stmt {
            Statement::Emit(ref range) => out.extend(range.clone()),
            Statement::Block(ref b) => collect_emitted(b, out),
            Statement::If {
                ref accept,
                ref reject,
                ..
            } => {
                collect_emitted(accept, out);
                collect_emitted(reject, out);
            }
            Statement::Switch { ref cases, .. } => {
                cases.iter().for_each(|c| collect_emitted(&c.body, out))
            }
            Statement::Loop {
                ref body,
                ref continuing,
                ..
            } => {
                collect_emitted(body, out);
                collect_emitted(continuing, out);
            }
            _ => {}
        }
    }
}
