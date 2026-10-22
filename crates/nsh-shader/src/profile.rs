//! NSH's profile: the WGSL every platform runs, checked on naga's IR.
//!
//! Android's runtime shaders (AGSL) are GLSL ES 1.00 with its Appendix A, so the profile is what
//! AGSL can say: `for` loops with constant bounds whose counter the body leaves alone, no `while` /
//! `loop`, no derivatives, `discard`, `u32` or bit operations, no resources of the shader's own, and
//! arrays indexed by constants or loop counters, which the checker proves in bounds. It also counts
//! the texture samples and loop iterations a pixel costs at most, and finds what the shader reads
//! (inputs, `nsh.time`, `nsh.progress`), for the limits and the reflection.

use std::collections::{HashMap, HashSet};

use naga::{
    BinaryOperator, Block, Expression, Function, Handle, Literal, LocalVariable, MathFunction,
    Module, ScalarKind, Statement, Type, TypeInner, UnaryOperator, VectorSize,
};

use crate::{Error, Input, Limits, layout};

/// The prelude's functions the app's code calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Builtin {
    /// `sample_content` / `sample_backdrop` / `sample_from` / `sample_to`.
    Sample(Input),
    /// `sample_image(image, p)`.
    SampleImage,
}

/// A `for` loop of the profile: `for (var i = start; i < bound; i += step)`.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(feature = "agsl"), allow(dead_code))]
pub(crate) struct CountedLoop {
    /// The counter.
    pub counter: Handle<LocalVariable>,
    /// Its kind: `Sint` or `Float`.
    pub kind: ScalarKind,
    pub start: f64,
    /// The comparison with the counter on the left.
    pub op: BinaryOperator,
    pub bound: f64,
    /// Added after each iteration (negative counts down).
    pub step: f64,
    pub iterations: u32,
    /// The counter's lowest and highest value inside the body.
    pub min: f64,
    pub max: f64,
    /// Statements at the start of the body that test the condition (the writer skips them).
    pub skip: usize,
}

/// What the checker found out about a shader.
#[derive(Debug, Clone)]
pub(crate) struct Analysis {
    /// The app's functions, in arena order (callees first).
    pub functions: Vec<Handle<Function>>,
    pub main: Handle<Function>,
    /// The struct `main` takes its uniforms in.
    pub uniforms: Option<Handle<Type>>,
    /// The prelude's `Image` struct.
    pub image: Option<Handle<Type>>,
    /// The global `nsh`.
    pub nsh: Option<Handle<naga::GlobalVariable>>,
    pub builtins: HashMap<Handle<Function>, Builtin>,
    /// Counted loops, by function and counter.
    pub loops: HashMap<(Handle<Function>, Handle<LocalVariable>), CountedLoop>,
    /// The inputs `main` samples (bit `Input::index`).
    pub inputs: u32,
    pub time: bool,
    pub progress: bool,
    /// Texture samples a pixel takes at most.
    pub samples: u32,
    /// Loop iterations a pixel runs at most.
    pub iterations: u32,
}

const NSH_TIME: u32 = 2;
const NSH_PROGRESS: u32 = 3;

pub(crate) fn user_span(module: &Module, f: Handle<Function>, user_len: usize) -> bool {
    module
        .functions
        .get_span(f)
        .to_range()
        .is_some_and(|r| r.start < user_len)
}

/// The app's `main` and the struct of its uniforms (by name), from the module with the stubs.
pub(crate) fn signature(module: &Module, source: &str) -> Result<Option<String>, Error> {
    let main = module
        .functions
        .iter()
        .find(|(h, f)| f.name.as_deref() == Some("main") && user_span(module, *h, source.len()))
        .ok_or_else(|| {
            Error::new("a shader has a `main`").note(
                "fn main(p: vec2f) -> vec4f, or fn main(p: vec2f, u: Uniforms) -> vec4f with a struct Uniforms",
            )
        })?;
    let (handle, function) = main;
    let span = module.functions.get_span(handle).to_range();
    let wrong = || {
        Error::at(
            "`main` takes `p: vec2f` and its uniforms' struct, and returns `vec4f`",
            source,
            span.clone(),
        )
        .note("fn main(p: vec2f) -> vec4f, or fn main(p: vec2f, u: Uniforms) -> vec4f")
    };
    let is_vec = |ty: Handle<Type>, size: VectorSize| matches!(module.types[ty].inner, TypeInner::Vector { size: s, scalar } if s == size && scalar == naga::Scalar::F32);
    if function.arguments.is_empty()
        || function.arguments.len() > 2
        || !is_vec(function.arguments[0].ty, VectorSize::Bi)
    {
        return Err(wrong());
    }
    match function.result {
        Some(ref result) if is_vec(result.ty, VectorSize::Quad) => {}
        _ => return Err(wrong()),
    }
    let Some(arg) = function.arguments.get(1) else {
        return Ok(None);
    };
    let ty = &module.types[arg.ty];
    let user_struct = matches!(ty.inner, TypeInner::Struct { .. })
        && module
            .types
            .get_span(arg.ty)
            .to_range()
            .is_some_and(|r| r.end <= source.len());
    match ty.name {
        Some(ref name) if user_struct => Ok(Some(name.clone())),
        _ => Err(wrong().note("the uniforms are a struct the shader declares")),
    }
}

struct Checker<'a> {
    module: &'a Module,
    info: &'a naga::valid::ModuleInfo,
    source: &'a str,
    limits: &'a Limits,
    analysis: Analysis,
    /// Image fields of the uniforms: field index -> slot.
    image_slots: HashMap<u32, u8>,
}

/// Checks the app's code in `module` (its source then `Glue::Check`) against the profile.
pub(crate) fn check(
    module: &Module,
    info: &naga::valid::ModuleInfo,
    source: &str,
    limits: &Limits,
) -> Result<Analysis, Error> {
    let user_len = source.len();
    let in_user = |span: naga::Span| span.to_range().is_some_and(|r| r.start < user_len);

    let named_type = |name: &str| {
        module
            .types
            .iter()
            .find(|(h, t)| t.name.as_deref() == Some(name) && !in_user(module.types.get_span(*h)))
            .map(|(h, _)| h)
    };
    let mut builtins = HashMap::new();
    let mut functions = Vec::new();
    let mut main = None;
    for (handle, function) in module.functions.iter() {
        let name = function.name.as_deref().unwrap_or_default();
        if in_user(module.functions.get_span(handle)) {
            if name.starts_with("nsh_") {
                return Err(Error::at(
                    format!("`{name}` is a name of NSH's (names starting with `nsh_` are)"),
                    source,
                    module.functions.get_span(handle).to_range(),
                ));
            }
            if name == "main" {
                main = Some(handle);
            }
            functions.push(handle);
            continue;
        }
        let builtin = match name {
            "sample_content" => Builtin::Sample(Input::Content),
            "sample_backdrop" => Builtin::Sample(Input::Backdrop),
            "sample_from" => Builtin::Sample(Input::From),
            "sample_to" => Builtin::Sample(Input::To),
            "sample_image" => Builtin::SampleImage,
            _ => continue,
        };
        builtins.insert(handle, builtin);
    }
    let main = main.ok_or_else(|| Error::new("a shader has a `main`"))?;
    let uniforms = module.functions[main].arguments.get(1).map(|a| a.ty);
    let nsh = module
        .global_variables
        .iter()
        .find(|(_, g)| g.name.as_deref() == Some("nsh"))
        .map(|(h, _)| h);

    let mut checker = Checker {
        module,
        info,
        source,
        limits,
        analysis: Analysis {
            functions,
            main,
            uniforms,
            image: named_type("Image"),
            nsh,
            builtins,
            loops: HashMap::new(),
            inputs: 0,
            time: false,
            progress: false,
            samples: 0,
            iterations: 0,
        },
        image_slots: HashMap::new(),
    };
    checker.module_scope()?;
    checker.uniforms()?;
    let functions = checker.analysis.functions.clone();
    for &f in &functions {
        checker.function(f)?;
    }
    checker.reach()?;
    Ok(checker.analysis)
}

/// A WGSL spelling of a type, for messages.
pub(crate) fn type_name(module: &Module, ty: Handle<Type>) -> String {
    let t = &module.types[ty];
    if let Some(ref name) = t.name {
        return name.clone();
    }
    let scalar = |s: naga::Scalar| match (s.kind, s.width) {
        (ScalarKind::Float, 4) => "f32",
        (ScalarKind::Float, 2) => "f16",
        (ScalarKind::Float, _) => "f64",
        (ScalarKind::Sint, 4) => "i32",
        (ScalarKind::Sint, _) => "i64",
        (ScalarKind::Uint, 4) => "u32",
        (ScalarKind::Uint, _) => "u64",
        (ScalarKind::Bool, _) => "bool",
        _ => "abstract",
    };
    match t.inner {
        TypeInner::Scalar(s) => scalar(s).into(),
        TypeInner::Vector { size, scalar: s } => format!("vec{}<{}>", size as u8, scalar(s)),
        TypeInner::Matrix {
            columns,
            rows,
            scalar: s,
        } => format!("mat{}x{}<{}>", columns as u8, rows as u8, scalar(s)),
        TypeInner::Array {
            base,
            size: naga::ArraySize::Constant(n),
            ..
        } => format!("array<{}, {n}>", type_name(module, base)),
        TypeInner::Array { base, .. } => format!("array<{}>", type_name(module, base)),
        TypeInner::Image { .. } => "texture".into(),
        TypeInner::Sampler { .. } => "sampler".into(),
        TypeInner::Pointer { base, .. } => format!("ptr<{}>", type_name(module, base)),
        _ => "type".into(),
    }
}

/// Where a type appears, for the rules that differ.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    /// A value of the app's code: locals, parameters, results, expressions.
    Value,
    /// A field of the uniforms' struct.
    Uniform,
    /// A field of a struct of the app's.
    Field,
    /// An array's element.
    Element,
}

impl Checker<'_> {
    fn error(&self, message: impl Into<String>, span: naga::Span) -> Error {
        Error::at(message, self.source, span.to_range())
    }

    fn in_user(&self, span: naga::Span) -> bool {
        span.to_range().is_some_and(|r| r.start < self.source.len())
    }

    /// Declarations at module scope: no resources, `var<private>` or overrides of the app's own.
    fn module_scope(&self) -> Result<(), Error> {
        let m = self.module;
        for (h, g) in m.global_variables.iter() {
            let span = m.global_variables.get_span(h);
            if self.in_user(span) {
                return Err(self
                    .error("a shader declares no global variables or resources of its own", span)
                    .note("its inputs are `nsh`, its uniforms' struct and the `sample_*` functions; constants are `const`")
                    .note(format!("`{}`", g.name.as_deref().unwrap_or("?"))));
            }
        }
        for (h, _) in m.overrides.iter() {
            let span = m.overrides.get_span(h);
            if self.in_user(span) {
                return Err(self.error(
                    "`override` is not in NSH's profile: use a `const` or a uniform",
                    span,
                ));
            }
        }
        for (h, c) in m.constants.iter() {
            let span = m.constants.get_span(h);
            if !self.in_user(span) {
                continue;
            }
            if let Some(ref name) = c.name
                && name.starts_with("nsh_")
            {
                return Err(self.error(
                    format!("`{name}` is a name of NSH's (names starting with `nsh_` are)"),
                    span,
                ));
            }
            self.ty(c.ty, Place::Value, span)?;
        }
        for (h, t) in m.types.iter() {
            let span = m.types.get_span(h);
            if !self.in_user(span) {
                continue;
            }
            if let Some(ref name) = t.name
                && (name.starts_with("nsh_") || name.starts_with("Nsh"))
            {
                return Err(self.error(format!("`{name}` is a name of NSH's"), span));
            }
            if let TypeInner::Struct { ref members, .. } = t.inner
                && Some(h) != self.analysis.uniforms
            {
                for member in members {
                    self.ty(member.ty, Place::Field, span)?;
                }
            }
        }
        Ok(())
    }

    /// The uniforms' fields: numbers, vectors, square matrices, arrays of them, images.
    fn uniforms(&mut self) -> Result<(), Error> {
        let Some(u) = self.analysis.uniforms else {
            return Ok(());
        };
        let span = self.module.types.get_span(u);
        let TypeInner::Struct { ref members, .. } = self.module.types[u].inner else {
            unreachable!()
        };
        let mut slot = 0u8;
        for (index, member) in members.iter().enumerate() {
            self.ty(member.ty, Place::Uniform, span)?;
            if Some(member.ty) == self.analysis.image {
                if slot as usize == layout::MAX_IMAGES {
                    return Err(self.error(
                        format!("a shader takes up to {} images", layout::MAX_IMAGES),
                        span,
                    ));
                }
                self.image_slots.insert(index as u32, slot);
                slot += 1;
            }
        }
        Ok(())
    }

    fn ty(&self, ty: Handle<Type>, place: Place, span: naga::Span) -> Result<(), Error> {
        let m = self.module;
        let name = || type_name(m, ty);
        let not = |what: String| Err(self.error(what, span));
        if Some(ty) == self.analysis.image {
            return match place {
                Place::Uniform => Ok(()),
                _ => not("an `Image` is a field of the uniforms' struct, read as `u.field`".into()),
            };
        }
        if Some(ty) == self.analysis.uniforms {
            return match place {
                Place::Value => Ok(()),
                _ => not(format!(
                    "the uniforms' struct `{}` is not a field or an element",
                    name()
                )),
            };
        }
        match m.types[ty].inner {
            TypeInner::Scalar(s) | TypeInner::Vector { scalar: s, .. } => {
                self.scalar(s, span, place == Place::Uniform)
            }
            TypeInner::Matrix {
                columns,
                rows,
                scalar,
            } => {
                if columns != rows || scalar != naga::Scalar::F32 {
                    return not(format!(
                        "`{}`: matrices are square and of f32 (mat2x2f, mat3x3f, mat4x4f)",
                        name()
                    ));
                }
                Ok(())
            }
            TypeInner::Array { base, size, .. } => {
                let naga::ArraySize::Constant(n) = size else {
                    return not("arrays have a constant size".into());
                };
                if n.get() > 1024 {
                    return not("arrays have up to 1024 elements".into());
                }
                match place {
                    Place::Element => not("arrays of arrays are not in NSH's profile".into()),
                    Place::Field => not(format!(
                        "`{}` in a struct: arrays are not fields of structs (except the uniforms')",
                        name()
                    )),
                    _ => {
                        if place == Place::Uniform
                            && matches!(m.types[base].inner, TypeInner::Struct { .. })
                        {
                            return not("uniforms are numbers, vectors, matrices, arrays of them and images".into());
                        }
                        self.ty(base, Place::Element, span)
                    }
                }
            }
            TypeInner::Struct { ref members, .. } => {
                if place == Place::Uniform {
                    return not(
                        "uniforms are numbers, vectors, matrices, arrays of them and images".into(),
                    );
                }
                let user = self.in_user(m.types.get_span(ty));
                if !user {
                    return not(format!("`{}` is NSH's own", name()));
                }
                for member in members {
                    self.ty(member.ty, Place::Field, span)?;
                }
                Ok(())
            }
            TypeInner::Pointer { .. } | TypeInner::ValuePointer { .. } => {
                not("pointers are not in NSH's profile: functions take and return values".into())
            }
            _ => not(format!("`{}` is not in NSH's profile", name())),
        }
    }

    fn scalar(&self, s: naga::Scalar, span: naga::Span, uniform: bool) -> Result<(), Error> {
        match (s.kind, s.width) {
            (ScalarKind::Float, 4) | (ScalarKind::Sint, 4) => Ok(()),
            (ScalarKind::Bool, _) if !uniform => Ok(()),
            (ScalarKind::Uint, _) => {
                Err(self.error("`u32` is not in NSH's profile: use `i32`", span))
            }
            (ScalarKind::Float, 2) => {
                Err(self.error("`f16` is not in NSH's profile: use `f32`", span))
            }
            _ => Err(self.error("NSH's profile has `f32`, `i32` and `bool`", span)),
        }
    }

    fn function(&mut self, f: Handle<Function>) -> Result<(), Error> {
        let m = self.module;
        let function = &m.functions[f];
        let fspan = m.functions.get_span(f);
        for arg in &function.arguments {
            if arg.binding.is_some() {
                return Err(self.error("functions of a shader take no bindings", fspan));
            }
            self.ty(arg.ty, Place::Value, fspan)?;
            if Some(arg.ty) == self.analysis.image {
                return Err(
                    self.error("an `Image` is not a parameter: read it as `u.field`", fspan)
                );
            }
        }
        if let Some(ref result) = function.result {
            self.ty(result.ty, Place::Value, fspan)?;
            if Some(result.ty) == self.analysis.uniforms {
                return Err(self.error("functions do not return the uniforms", fspan));
            }
            if matches!(m.types[result.ty].inner, TypeInner::Array { .. }) {
                return Err(self.error("functions do not return arrays in NSH's profile", fspan));
            }
        }
        for (h, var) in function.local_variables.iter() {
            let span = function.local_variables.get_span(h);
            self.ty(var.ty, Place::Value, span)?;
            if Some(var.ty) == self.analysis.uniforms {
                return Err(self.error(
                    "the uniforms are read where they are (`u.field`), not copied to a `var`",
                    span,
                ));
            }
        }
        self.expressions(f)?;

        // Counted loops first: uses of their counters are checked against them.
        let mut loops = Vec::new();
        self.find_loops(f, &function.body, 0, &mut loops)?;
        for l in loops {
            self.analysis.loops.insert((f, l.counter), l);
        }
        let mut active = Vec::new();
        self.block(f, &function.body, 0, &mut active, false)?;
        Ok(())
    }

    /// Rules on single expressions.
    fn expressions(&self, f: Handle<Function>) -> Result<(), Error> {
        let m = self.module;
        let function = &m.functions[f];
        for (h, e) in function.expressions.iter() {
            let span = function.expressions.get_span(h);
            let err = |what: &str| Err(self.error(what.to_string(), span));
            match *e {
                Expression::Literal(l) => match l {
                    Literal::F32(_) | Literal::I32(_) | Literal::Bool(_) => {}
                    Literal::U32(_) => return err("`u32` is not in NSH's profile: use `i32`"),
                    Literal::AbstractInt(_) | Literal::AbstractFloat(_) => {}
                    _ => return err("NSH's profile has `f32`, `i32` and `bool`"),
                },
                Expression::Override(_) => return err("`override` is not in NSH's profile"),
                Expression::ZeroValue(ty) | Expression::Compose { ty, .. } => {
                    if Some(ty) == self.analysis.uniforms || Some(ty) == self.analysis.image {
                        return err(
                            "the uniforms and their images come from NSH, a shader does not make them",
                        );
                    }
                    self.ty(ty, Place::Value, span)?;
                }
                Expression::AccessIndex { base, index } => {
                    if index != 0
                        && self.analysis.image.is_some()
                        && self.value_type(f, base) == self.analysis.image
                    {
                        return err("an `Image` has one field, `size`");
                    }
                }
                Expression::GlobalVariable(g) => {
                    if Some(g) != self.analysis.nsh {
                        let name = m.global_variables[g].name.as_deref().unwrap_or("?");
                        return err(&format!("`{name}` is NSH's own"));
                    }
                }
                Expression::Load { pointer } => {
                    if let Expression::GlobalVariable(_) = function.expressions[pointer] {
                        return err(
                            "read the fields of `nsh` (nsh.size, nsh.scale, nsh.time, nsh.progress)",
                        );
                    }
                }
                Expression::ImageSample { .. }
                | Expression::ImageLoad { .. }
                | Expression::ImageQuery { .. } => {
                    return err("a shader samples its inputs with the `sample_*` functions");
                }
                Expression::Unary {
                    op: UnaryOperator::BitwiseNot,
                    ..
                } => {
                    if self.resolve(f, h).scalar_kind() != Some(ScalarKind::Bool) {
                        return err("bit operations are not in NSH's profile");
                    }
                }
                Expression::Unary { .. } => {}
                Expression::Binary { op, left, .. } => match op {
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
                        return err("bit operations are not in NSH's profile");
                    }
                    BinaryOperator::And
                    | BinaryOperator::InclusiveOr
                    | BinaryOperator::ExclusiveOr
                        if self.resolve(f, left).scalar_kind() != Some(ScalarKind::Bool) =>
                    {
                        return err("bit operations are not in NSH's profile");
                    }
                    _ => {}
                },
                Expression::Derivative { .. } => {
                    return err("derivatives (dpdx, dpdy, fwidth) are not in NSH's profile");
                }
                Expression::Relational { fun, .. } => match fun {
                    naga::RelationalFunction::All | naga::RelationalFunction::Any => {}
                    _ => return err("this function is not in NSH's profile"),
                },
                Expression::Math { fun, arg, .. } => {
                    use MathFunction as Mf;
                    match fun {
                        Mf::Abs
                        | Mf::Min
                        | Mf::Max
                        | Mf::Clamp
                        | Mf::Saturate
                        | Mf::Cos
                        | Mf::Cosh
                        | Mf::Sin
                        | Mf::Sinh
                        | Mf::Tan
                        | Mf::Tanh
                        | Mf::Acos
                        | Mf::Asin
                        | Mf::Atan
                        | Mf::Atan2
                        | Mf::Asinh
                        | Mf::Acosh
                        | Mf::Atanh
                        | Mf::Radians
                        | Mf::Degrees
                        | Mf::Ceil
                        | Mf::Floor
                        | Mf::Round
                        | Mf::Fract
                        | Mf::Trunc
                        | Mf::Exp
                        | Mf::Exp2
                        | Mf::Log
                        | Mf::Log2
                        | Mf::Pow
                        | Mf::Dot
                        | Mf::Cross
                        | Mf::Distance
                        | Mf::Length
                        | Mf::Normalize
                        | Mf::FaceForward
                        | Mf::Reflect
                        | Mf::Refract
                        | Mf::Sign
                        | Mf::Fma
                        | Mf::Mix
                        | Mf::Step
                        | Mf::SmoothStep
                        | Mf::Sqrt
                        | Mf::InverseSqrt
                        | Mf::Transpose
                        | Mf::Determinant => {}
                        _ => {
                            return err(
                                "this function is not in NSH's profile (bit, packing and decomposition functions are not)",
                            );
                        }
                    }
                    let _ = arg;
                }
                Expression::As { convert: None, .. } => {
                    return err("`bitcast` is not in NSH's profile");
                }
                Expression::As { .. }
                | Expression::Constant(_)
                | Expression::Access { .. }
                | Expression::Splat { .. }
                | Expression::Swizzle { .. }
                | Expression::FunctionArgument(_)
                | Expression::LocalVariable(_)
                | Expression::Select { .. }
                | Expression::CallResult(_) => {}
                _ => return err("this expression is not in NSH's profile"),
            }
        }
        Ok(())
    }

    fn resolve(&self, f: Handle<Function>, h: Handle<Expression>) -> &TypeInner {
        self.info[f][h].ty.inner_with(&self.module.types)
    }

    /// The named type of an expression's value, or of what a pointer expression points to.
    fn value_type(&self, f: Handle<Function>, h: Handle<Expression>) -> Option<Handle<Type>> {
        match self.info[f][h].ty {
            naga::proc::TypeResolution::Handle(ty) => Some(ty),
            naga::proc::TypeResolution::Value(TypeInner::Pointer { base, .. }) => Some(base),
            _ => None,
        }
    }

    /// Finds the counted loops of `block` and checks every loop is one.
    fn find_loops(
        &self,
        f: Handle<Function>,
        block: &Block,
        depth: u32,
        out: &mut Vec<CountedLoop>,
    ) -> Result<(), Error> {
        for (i, (stmt, span)) in block.span_iter().enumerate() {
            match *stmt {
                Statement::Loop {
                    ref body,
                    ref continuing,
                    break_if,
                } => {
                    let l =
                        self.counted_loop(f, block, i, body, continuing, break_if, depth, *span)?;
                    out.push(l);
                    self.find_loops(f, body, depth + 1, out)?;
                }
                Statement::Block(ref b) => self.find_loops(f, b, depth, out)?,
                Statement::If {
                    ref accept,
                    ref reject,
                    ..
                } => {
                    self.find_loops(f, accept, depth, out)?;
                    self.find_loops(f, reject, depth, out)?;
                }
                Statement::Switch { ref cases, .. } => {
                    for case in cases {
                        self.find_loops(f, &case.body, depth, out)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn counted_loop(
        &self,
        f: Handle<Function>,
        parent: &Block,
        index: usize,
        body: &Block,
        continuing: &Block,
        break_if: Option<Handle<Expression>>,
        depth: u32,
        span: naga::Span,
    ) -> Result<CountedLoop, Error> {
        let m = self.module;
        let function = &m.functions[f];
        let not = || {
            self.error("loops are `for` loops with constant bounds in NSH's profile", span)
                .note("for (var i = 0; i < 8; i++) { ... }: a constant start, bound and step, and a body that leaves `i` alone")
        };
        if break_if.is_some() {
            return Err(not());
        }
        let shape = loop_shape(function, body, continuing).ok_or_else(not)?;
        let counter = shape.counter;
        let var = &function.local_variables[counter];
        let kind = match m.types[var.ty].inner {
            TypeInner::Scalar(s) if matches!(s.kind, ScalarKind::Sint | ScalarKind::Float) => {
                s.kind
            }
            _ => return Err(not()),
        };
        let bound = const_value(m, function, shape.bound)
            .ok_or_else(|| not().note("the bound is a constant"))?;
        let step = const_value(m, function, shape.step)
            .ok_or_else(|| not().note("the step is a constant"))?;
        let step = if shape.step_op == BinaryOperator::Subtract {
            -step
        } else {
            step
        };

        // The start: the `Store` right before the loop, or the variable's constant initializer
        // when the loop is not inside another (naga hoists those to the function's start).
        let before = parent
            .iter()
            .take(index)
            .rev()
            .find(|s| !matches!(s, Statement::Emit(_)));
        let start = match before {
            Some(&Statement::Store { pointer, value }) if matches!(function.expressions[pointer], Expression::LocalVariable(v) if v == counter) => {
                const_value(m, function, value)
            }
            _ if depth == 0 => match var.init {
                Some(init) => const_value(m, function, init),
                None => Some(0.0),
            },
            _ => None,
        }
        .ok_or_else(|| not().note("the counter starts at a constant: for (var i = 0; ...)"))?;

        // Run the loop: counts, and the range of the counter.
        let (mut x, mut n, mut min, mut max) = (start, 0u32, f64::INFINITY, f64::NEG_INFINITY);
        let as_kind = |v: f64| {
            if kind == ScalarKind::Float {
                v as f32 as f64
            } else {
                v
            }
        };
        let limit = self.limits.max_iterations.max(1);
        loop {
            let go = match shape.op {
                BinaryOperator::Less => x < bound,
                BinaryOperator::LessEqual => x <= bound,
                BinaryOperator::Greater => x > bound,
                BinaryOperator::GreaterEqual => x >= bound,
                BinaryOperator::NotEqual => x != bound,
                _ => return Err(not()),
            };
            if !go {
                break;
            }
            n += 1;
            min = min.min(x);
            max = max.max(x);
            if n > limit
                || (kind == ScalarKind::Sint
                    && !(i32::MIN as f64..=i32::MAX as f64).contains(&(x + step)))
            {
                return Err(self.error(
                    format!("this loop runs more than {limit} times (a pixel runs at most {limit} iterations)"),
                    span,
                ));
            }
            x = as_kind(x + step);
        }
        Ok(CountedLoop {
            counter,
            kind,
            start,
            op: shape.op,
            bound,
            step,
            iterations: n,
            min,
            max,
            skip: shape.skip,
        })
    }

    /// Statement rules, with the counted loops `active` around them (and whether a `switch` is the
    /// nearest construct a `break` would leave).
    fn block(
        &mut self,
        f: Handle<Function>,
        block: &Block,
        skip: usize,
        active: &mut Vec<Handle<LocalVariable>>,
        in_switch: bool,
    ) -> Result<(), Error> {
        let m = self.module;
        let function = &m.functions[f];
        for (i, (stmt, span)) in block.span_iter().enumerate().skip(skip) {
            let span = *span;
            match *stmt {
                Statement::Emit(ref range) => {
                    for h in range.clone() {
                        self.emitted(f, h, active)?;
                    }
                }
                Statement::Block(ref b) => self.block(f, b, 0, active, in_switch)?,
                Statement::If {
                    ref accept,
                    ref reject,
                    ..
                } => {
                    self.block(f, accept, 0, active, in_switch)?;
                    self.block(f, reject, 0, active, in_switch)?;
                }
                Statement::Switch { ref cases, .. } => {
                    for case in cases {
                        self.block(f, &case.body, 0, active, true)?;
                    }
                }
                Statement::Loop {
                    ref body,
                    ref continuing,
                    ..
                } => {
                    // `find_loops` returned an error for any other loop.
                    let counter = loop_shape(function, body, continuing)
                        .expect("a counted loop")
                        .counter;
                    let skip = self.analysis.loops[&(f, counter)].skip;
                    active.push(counter);
                    // The condition's statements are the loop's own; the body is what follows.
                    self.block(f, body, skip, active, false)?;
                    active.pop();
                }
                Statement::Break if in_switch => {
                    return Err(
                        self.error("`break` does not leave a `switch` in NSH's profile", span)
                    );
                }
                Statement::Break | Statement::Continue | Statement::Return { .. } => {}
                Statement::Kill => {
                    return Err(self.error(
                        "`discard` is not in NSH's profile: return transparent",
                        span,
                    ));
                }
                Statement::Store { pointer, .. } => {
                    if let Some(v) = root_local(function, pointer)
                        && self.analysis.loops.contains_key(&(f, v))
                    {
                        // The init store before the loop is the loop's own.
                        let next = block
                            .iter()
                            .skip(i + 1)
                            .find(|s| !matches!(s, Statement::Emit(_)));
                        let init = matches!(next, Some(Statement::Loop { body, continuing, .. })
                            if loop_shape(function, body, continuing).is_some_and(|s| s.counter == v));
                        if !init {
                            return Err(self
                                .error("the body of a `for` loop leaves its counter alone", span));
                        }
                    }
                }
                Statement::Call {
                    function: callee,
                    ref arguments,
                    ..
                } => {
                    if !self.analysis.builtins.contains_key(&callee)
                        && !self.analysis.functions.contains(&callee)
                    {
                        let name = m.functions[callee].name.as_deref().unwrap_or("?");
                        return Err(self.error(format!("`{name}` is NSH's own"), span));
                    }
                    for &a in arguments {
                        self.emitted(f, a, active)?;
                    }
                }
                _ => return Err(self.error("this statement is not in NSH's profile", span)),
            }
        }
        Ok(())
    }

    /// Rules on an expression where it is evaluated: counters only inside their loops, indexes in
    /// bounds.
    fn emitted(
        &self,
        f: Handle<Function>,
        h: Handle<Expression>,
        active: &[Handle<LocalVariable>],
    ) -> Result<(), Error> {
        let m = self.module;
        let function = &m.functions[f];
        let span = function.expressions.get_span(h);
        match function.expressions[h] {
            Expression::Load { pointer } => {
                if let Some(v) = root_local(function, pointer)
                    && self.analysis.loops.contains_key(&(f, v))
                    && !active.contains(&v)
                {
                    return Err(
                        self.error("a `for` loop's counter is read only inside the loop", span)
                    );
                }
            }
            Expression::Access { base, index } => {
                let len = match *self.resolve(f, base) {
                    TypeInner::Pointer { base, .. } => indexable_len(&m.types[base].inner),
                    ref inner => indexable_len(inner),
                };
                let Some(len) = len else { return Ok(()) };
                let (lo, hi) = self.index_range(f, index, active).ok_or_else(|| {
                    self.error(
                        "an index is a constant or a loop counter (with constants: i + 1, 2 * i)",
                        span,
                    )
                })?;
                if lo < 0.0 || hi > (len - 1) as f64 {
                    return Err(self.error(
                        format!("this index goes from {lo} to {hi}, out of 0..{}", len - 1),
                        span,
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The values an index takes: constants and active loop counters with `+`, `-`, `*`.
    fn index_range(
        &self,
        f: Handle<Function>,
        h: Handle<Expression>,
        active: &[Handle<LocalVariable>],
    ) -> Option<(f64, f64)> {
        let m = self.module;
        let function = &m.functions[f];
        if let Some(v) = const_value(m, function, h) {
            return Some((v, v));
        }
        match function.expressions[h] {
            Expression::Load { pointer } => {
                let Expression::LocalVariable(v) = function.expressions[pointer] else {
                    return None;
                };
                if !active.contains(&v) {
                    return None;
                }
                let l = self.analysis.loops.get(&(f, v))?;
                (l.kind == ScalarKind::Sint).then_some(if l.iterations == 0 {
                    (0.0, 0.0)
                } else {
                    (l.min, l.max)
                })
            }
            Expression::Binary { op, left, right } => {
                let (a, b) = self.index_range(f, left, active)?;
                let (c, d) = self.index_range(f, right, active)?;
                match op {
                    BinaryOperator::Add => Some((a + c, b + d)),
                    BinaryOperator::Subtract => Some((a - d, b - c)),
                    BinaryOperator::Multiply => {
                        let p = [a * c, a * d, b * c, b * d];
                        Some((
                            p.iter().cloned().fold(f64::INFINITY, f64::min),
                            p.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                        ))
                    }
                    _ => None,
                }
            }
            Expression::Unary {
                op: UnaryOperator::Negate,
                expr,
            } => {
                let (a, b) = self.index_range(f, expr, active)?;
                Some((-b, -a))
            }
            _ => None,
        }
    }

    /// What `main` reaches: inputs, `nsh.time` / `nsh.progress`, the cost of a pixel.
    fn reach(&mut self) -> Result<(), Error> {
        let m = self.module;
        let mut cost: HashMap<Handle<Function>, (u64, u64)> = HashMap::new();
        let mut inputs: HashMap<Handle<Function>, u32> = HashMap::new();
        let mut time: HashSet<Handle<Function>> = HashSet::new();
        let mut progress: HashSet<Handle<Function>> = HashSet::new();
        for &f in &self.analysis.functions {
            let function = &m.functions[f];
            let mut used = 0u32;
            let mut reads_time = false;
            let mut reads_progress = false;
            for (_, e) in function.expressions.iter() {
                if let Expression::AccessIndex { base, index } = *e
                    && let Expression::GlobalVariable(g) = function.expressions[base]
                    && Some(g) == self.analysis.nsh
                {
                    reads_time |= index == NSH_TIME;
                    reads_progress |= index == NSH_PROGRESS;
                }
            }
            let c = self.cost(
                f,
                &function.body,
                &cost,
                &mut used,
                &inputs,
                &mut reads_time,
                &mut reads_progress,
                &time,
                &progress,
            )?;
            cost.insert(f, c);
            inputs.insert(f, used);
            if reads_time {
                time.insert(f);
            }
            if reads_progress {
                progress.insert(f);
            }
        }
        let main = self.analysis.main;
        let (samples, iterations) = cost[&main];
        self.analysis.inputs = inputs[&main];
        self.analysis.time = time.contains(&main);
        self.analysis.progress = progress.contains(&main);
        self.analysis.samples = samples.min(u32::MAX as u64) as u32;
        self.analysis.iterations = iterations.min(u32::MAX as u64) as u32;
        let span = m.functions.get_span(main);
        if samples > self.limits.max_samples as u64 {
            return Err(self
                .error(
                    format!(
                        "a pixel of this shader takes up to {samples} samples, more than {}",
                        self.limits.max_samples
                    ),
                    span,
                )
                .note("loops multiply the samples in their bodies"));
        }
        if iterations > self.limits.max_iterations as u64 {
            return Err(self.error(
                format!(
                    "a pixel of this shader runs up to {iterations} loop iterations, more than {}",
                    self.limits.max_iterations
                ),
                span,
            ));
        }
        Ok(())
    }

    /// The samples and iterations `stmts` cost at most: the costlier path, a branch that returns
    /// not followed by what comes after it.
    #[allow(clippy::too_many_arguments)]
    fn cost(
        &self,
        f: Handle<Function>,
        stmts: &[Statement],
        costs: &HashMap<Handle<Function>, (u64, u64)>,
        used: &mut u32,
        inputs: &HashMap<Handle<Function>, u32>,
        reads_time: &mut bool,
        reads_progress: &mut bool,
        time: &HashSet<Handle<Function>>,
        progress: &HashSet<Handle<Function>>,
    ) -> Result<(u64, u64), Error> {
        let m = self.module;
        let function = &m.functions[f];
        let Some((head, rest)) = stmts.split_first() else {
            return Ok((0, 0));
        };
        let sub = |b: &[Statement], used: &mut u32, rt: &mut bool, rp: &mut bool| {
            self.cost(f, b, costs, used, inputs, rt, rp, time, progress)
        };
        let add = |a: (u64, u64), b: (u64, u64)| (a.0.saturating_add(b.0), a.1.saturating_add(b.1));
        let max = |a: (u64, u64), b: (u64, u64)| (a.0.max(b.0), a.1.max(b.1));
        let after = sub(rest, used, reads_time, reads_progress)?;
        Ok(match *head {
            Statement::Return { .. } | Statement::Kill => (0, 0),
            Statement::Block(ref b) if returns(b) => sub(b, used, reads_time, reads_progress)?,
            Statement::Block(ref b) => add(sub(b, used, reads_time, reads_progress)?, after),
            Statement::If {
                ref accept,
                ref reject,
                ..
            } => {
                let branch = |b: &Block,
                              used: &mut u32,
                              rt: &mut bool,
                              rp: &mut bool|
                 -> Result<(u64, u64), Error> {
                    let c = sub(b, used, rt, rp)?;
                    Ok(if returns(b) { c } else { add(c, after) })
                };
                let a = branch(accept, used, reads_time, reads_progress)?;
                let r = branch(reject, used, reads_time, reads_progress)?;
                max(a, r)
            }
            Statement::Switch { ref cases, .. } => {
                let mut c = (0, 0);
                for case in cases {
                    let k = sub(&case.body, used, reads_time, reads_progress)?;
                    c = max(
                        c,
                        if returns(&case.body) {
                            k
                        } else {
                            add(k, after)
                        },
                    );
                }
                c
            }
            Statement::Loop {
                ref body,
                ref continuing,
                ..
            } => {
                let shape = loop_shape(function, body, continuing).expect("checked");
                let n = self.analysis.loops[&(f, shape.counter)].iterations as u64;
                let b = sub(body, used, reads_time, reads_progress)?;
                add((n.saturating_mul(b.0), n.saturating_mul(1 + b.1)), after)
            }
            Statement::Call {
                function: callee,
                ref arguments,
                ..
            } => {
                let c = match self.analysis.builtins.get(&callee) {
                    Some(&Builtin::Sample(input)) => {
                        *used |= 1 << input.index();
                        (1, 0)
                    }
                    Some(Builtin::SampleImage) => {
                        let slot = self.image_slot(f, arguments[0])?;
                        *used |= 1 << Input::Image(slot).index();
                        (1, 0)
                    }
                    None => {
                        *used |= inputs.get(&callee).copied().unwrap_or(0);
                        *reads_time |= time.contains(&callee);
                        *reads_progress |= progress.contains(&callee);
                        costs.get(&callee).copied().unwrap_or((0, 0))
                    }
                };
                add(c, after)
            }
            _ => after,
        })
    }

    /// The slot of the image `u.field` names.
    fn image_slot(&self, f: Handle<Function>, h: Handle<Expression>) -> Result<u8, Error> {
        let function = &self.module.functions[f];
        if let Expression::AccessIndex { index, .. } = function.expressions[h]
            && let Some(&slot) = self.image_slots.get(&index)
        {
            return Ok(slot);
        }
        Err(self.error(
            "`sample_image` samples an image of the uniforms: sample_image(u.field, p)",
            function.expressions.get_span(h),
        ))
    }
}

/// Whether every path through `block` returns.
pub(crate) fn returns(block: &Block) -> bool {
    match block.last() {
        Some(Statement::Return { .. } | Statement::Kill) => true,
        Some(Statement::Block(b)) => returns(b),
        Some(Statement::If { accept, reject, .. }) => returns(accept) && returns(reject),
        Some(Statement::Switch { cases, .. }) => {
            cases.iter().any(|c| c.value == naga::SwitchValue::Default)
                && cases.iter().all(|c| c.fall_through || returns(&c.body))
        }
        _ => false,
    }
}

/// The parts of a `for` loop in naga's IR: `body` starts with the condition (`if !(i < n) { break; }`),
/// `continuing` is the update (`i = i + step`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct LoopShape {
    pub counter: Handle<LocalVariable>,
    pub op: BinaryOperator,
    pub bound: Handle<Expression>,
    pub step_op: BinaryOperator,
    pub step: Handle<Expression>,
    pub skip: usize,
}

pub(crate) fn loop_shape(
    function: &Function,
    body: &Block,
    continuing: &Block,
) -> Option<LoopShape> {
    let exprs = &function.expressions;
    let counter_of = |h: Handle<Expression>| match exprs[h] {
        Expression::Load { pointer } => match exprs[pointer] {
            Expression::LocalVariable(v) => Some(v),
            _ => None,
        },
        _ => None,
    };
    // The condition: `if c {} else { break; }` (WGSL's `for` and `while`), or `if !c { break; }`
    // and `if c { break; }` (a `loop` written out, as naga's WGSL writer does).
    let mut skip = 0;
    let mut condition = None;
    for stmt in body.iter() {
        skip += 1;
        match *stmt {
            Statement::Emit(_) => continue,
            Statement::If {
                condition: c,
                ref accept,
                ref reject,
            } => {
                let is_break = |b: &Block| matches!(b.iter().as_slice(), [Statement::Break]);
                if accept.is_empty() && is_break(reject) {
                    condition = Some((c, false));
                } else if reject.is_empty() && is_break(accept) {
                    condition = Some((c, true));
                }
            }
            _ => {}
        }
        break;
    }
    let (mut c, mut negated) = condition?;
    if negated
        && let Expression::Unary {
            op: UnaryOperator::LogicalNot,
            expr,
        } = exprs[c]
    {
        c = expr;
        negated = false;
    }
    let Expression::Binary { op, left, right } = exprs[c] else {
        return None;
    };
    // The loop goes on while the condition of its `break` is false.
    let op = if negated {
        match op {
            BinaryOperator::Less => BinaryOperator::GreaterEqual,
            BinaryOperator::LessEqual => BinaryOperator::Greater,
            BinaryOperator::Greater => BinaryOperator::LessEqual,
            BinaryOperator::GreaterEqual => BinaryOperator::Less,
            BinaryOperator::Equal => BinaryOperator::NotEqual,
            _ => return None,
        }
    } else {
        op
    };
    let flip = |op| match op {
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessEqual => BinaryOperator::GreaterEqual,
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterEqual => BinaryOperator::LessEqual,
        op => op,
    };
    let (counter, op, bound) = match (counter_of(left), counter_of(right)) {
        (Some(v), _) => (v, op, right),
        (None, Some(v)) => (v, flip(op), left),
        _ => return None,
    };
    if !matches!(
        op,
        BinaryOperator::Less
            | BinaryOperator::LessEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterEqual
            | BinaryOperator::NotEqual
    ) {
        return None;
    }
    let mut update = continuing
        .iter()
        .filter(|s| !matches!(s, Statement::Emit(_)));
    let Some(&Statement::Store { pointer, value }) = update.next() else {
        return None;
    };
    if update.next().is_some()
        || !matches!(exprs[pointer], Expression::LocalVariable(v) if v == counter)
    {
        return None;
    }
    let Expression::Binary {
        op: step_op,
        left,
        right,
    } = exprs[value]
    else {
        return None;
    };
    let step = match (step_op, counter_of(left), counter_of(right)) {
        (BinaryOperator::Add | BinaryOperator::Subtract, Some(v), _) if v == counter => right,
        (BinaryOperator::Add, _, Some(v)) if v == counter => left,
        _ => return None,
    };
    Some(LoopShape {
        counter,
        op,
        bound,
        step_op,
        step,
        skip,
    })
}

/// The local variable a pointer expression points into.
pub(crate) fn root_local(
    function: &Function,
    mut h: Handle<Expression>,
) -> Option<Handle<LocalVariable>> {
    loop {
        match function.expressions[h] {
            Expression::LocalVariable(v) => return Some(v),
            Expression::Access { base, .. } | Expression::AccessIndex { base, .. } => h = base,
            _ => return None,
        }
    }
}

fn indexable_len(inner: &TypeInner) -> Option<u32> {
    match *inner {
        TypeInner::Vector { size, .. }
        | TypeInner::ValuePointer {
            size: Some(size), ..
        } => Some(size as u32),
        TypeInner::Matrix { columns, .. } => Some(columns as u32),
        TypeInner::Array {
            size: naga::ArraySize::Constant(n),
            ..
        } => Some(n.get()),
        _ => None,
    }
}

/// The value of a constant scalar expression.
pub(crate) fn const_value(
    module: &Module,
    function: &Function,
    h: Handle<Expression>,
) -> Option<f64> {
    eval(module, &function.expressions, h)
}

fn eval(module: &Module, arena: &naga::Arena<Expression>, h: Handle<Expression>) -> Option<f64> {
    match arena[h] {
        Expression::Literal(l) => match l {
            Literal::F32(v) => Some(v as f64),
            Literal::I32(v) => Some(v as f64),
            Literal::AbstractInt(v) => Some(v as f64),
            Literal::AbstractFloat(v) => Some(v),
            _ => None,
        },
        Expression::Constant(c) => {
            eval(module, &module.global_expressions, module.constants[c].init)
        }
        Expression::ZeroValue(ty) => {
            matches!(module.types[ty].inner, TypeInner::Scalar(_)).then_some(0.0)
        }
        Expression::Unary {
            op: UnaryOperator::Negate,
            expr,
        } => eval(module, arena, expr).map(|v| -v),
        _ => None,
    }
}
