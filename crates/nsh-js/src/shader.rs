//! Shaders' JS face (`docs/dev/shaders.md`): `createShader(source)`, which
//! checks a WGSL source in Rust and returns the object `<Shader src>`
//! takes, and `shaderSupport()`.
//!
//! A VM keeps the shaders it made ([`Shaders`]), by id and by source: the
//! same source (a module run again by hot module replacement, a component
//! making its shader on each render) is the same shader, compiled once
//! and counted once against `max_shaders`. The object holds the id
//! (`__shader`, not enumerable), which `src` decodes back
//! ([`lookup`], `HostInput::shader`), and the uniforms' names.

use std::{collections::HashMap, ffi::CString, os::raw::c_int, sync::Arc};

use nsh_core::shader::{ShaderLimits, ShaderSource, Support};
use rquickjs::qjs;

use crate::{dom, reactive::arg};

type JsValue = qjs::JSValue;
type JsContext = qjs::JSContext;

/// The shaders a VM made.
#[derive(Default)]
pub(crate) struct Shaders {
    by_id: HashMap<u32, ShaderSource>,
    by_source: HashMap<Arc<str>, u32>,
}

/// The shader `id` names in the VM of `cx`.
pub(crate) fn lookup(cx: *mut JsContext, id: u32) -> Option<ShaderSource> {
    dom::shared(cx).shaders.borrow().by_id.get(&id).cloned()
}

/// `createShader(source)`.
pub(crate) unsafe extern "C" fn create_shader(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    let source = arg(argc, argv, 0);
    // SAFETY: a live argument.
    if !unsafe { qjs::JS_IsString(source) } {
        return throw(
            cx,
            Kind::Type,
            "createShader: the source is a string of WGSL",
        );
    }
    let Some(source) = dom::string_arg(cx, source) else {
        return qjs::JS_EXCEPTION;
    };
    let shared = dom::shared(cx);
    let known = shared
        .shaders
        .borrow()
        .by_source
        .get(source.as_str())
        .copied();
    let shader = match known {
        Some(id) => shared.shaders.borrow().by_id[&id].clone(),
        None => {
            let limits = shared.limits;
            if shared.shaders.borrow().by_id.len() >= limits.max_shaders {
                let message = format!(
                    "createShader: an app makes up to {} shaders",
                    limits.max_shaders
                );
                return throw(cx, Kind::Range, &message);
            }
            let mut shader_limits = ShaderLimits::default();
            shader_limits.max_samples = limits.max_shader_samples;
            let compiled = match nsh_core::shader::compile(&source, &shader_limits) {
                Ok(s) => s,
                Err(e) => {
                    let mut message = format!("createShader: {}", e.render("shader.wgsl", &source));
                    while message.ends_with('\n') {
                        message.pop();
                    }
                    return throw(cx, Kind::Syntax, &message);
                }
            };
            let shader = ShaderSource::with_id(ShaderSource::next_id(), Arc::new(compiled));
            let mut shaders = shared.shaders.borrow_mut();
            shaders.by_source.insert(source.into(), shader.id);
            shaders.by_id.insert(shader.id, shader.clone());
            shader
        }
    };
    object(cx, &shader)
}

/// `{ uniforms: [names], __shader: id }`, frozen.
fn object(cx: *mut JsContext, shader: &ShaderSource) -> JsValue {
    // SAFETY: new values on a live context, handed to the object.
    unsafe {
        let obj = qjs::JS_NewObject(cx);
        let names = qjs::JS_NewArray(cx);
        for (i, u) in shader.shader.reflection().uniforms.iter().enumerate() {
            let name = qjs::JS_NewStringLen(cx, u.name.as_ptr().cast(), u.name.len() as _);
            qjs::JS_SetPropertyUint32(cx, names, i as u32, name);
        }
        qjs::JS_FreezeObject(cx, names);
        qjs::JS_DefinePropertyValueStr(
            cx,
            obj,
            c"uniforms".as_ptr(),
            names,
            qjs::JS_PROP_ENUMERABLE as c_int,
        );
        qjs::JS_DefinePropertyValueStr(
            cx,
            obj,
            c"__shader".as_ptr(),
            qjs::JS_NewFloat64(shader.id as f64),
            0,
        );
        qjs::JS_FreezeObject(cx, obj);
        obj
    }
}

/// `shaderSupport()`: `{ fill, backdrop, effect, transition, screenEffect,
/// screenTransition }`, each
/// `"live"`, `"snapshot"` or `false`.
pub(crate) unsafe extern "C" fn shader_support(
    cx: *mut JsContext,
    _this: JsValue,
    _argc: c_int,
    _argv: *mut JsValue,
) -> JsValue {
    let support = crate::env::current(cx).shaders;
    let value = |s: Support| match s.js_name() {
        // SAFETY: a new string on a live context.
        Some(name) => unsafe { qjs::JS_NewStringLen(cx, name.as_ptr().cast(), name.len() as _) },
        None => qjs::JS_FALSE,
    };
    // SAFETY: a new object; the values are handed to it.
    unsafe {
        let obj = qjs::JS_NewObject(cx);
        for (key, s) in [
            (c"fill", support.fill),
            (c"backdrop", support.backdrop),
            (c"effect", support.effect),
            (c"transition", support.transition),
            (c"screenEffect", support.screen_effect),
            (c"screenTransition", support.screen_transition),
        ] {
            qjs::JS_SetPropertyStr(cx, obj, key.as_ptr(), value(s));
        }
        obj
    }
}

enum Kind {
    Type,
    Range,
    Syntax,
}

fn throw(cx: *mut JsContext, kind: Kind, message: &str) -> JsValue {
    let message = CString::new(message.replace('\0', " ")).unwrap_or_default();
    // SAFETY: throws on a live context; `%s` keeps the message literal.
    unsafe {
        match kind {
            Kind::Type => qjs::JS_ThrowTypeError(cx, c"%s".as_ptr(), message.as_ptr()),
            Kind::Range => qjs::JS_ThrowRangeError(cx, c"%s".as_ptr(), message.as_ptr()),
            Kind::Syntax => qjs::JS_ThrowSyntaxError(cx, c"%s".as_ptr(), message.as_ptr()),
        }
    }
}
