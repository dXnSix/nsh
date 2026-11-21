//! Pictures of a window's content drawn in the app's process, for a design
//! tool (`nsh/design`'s `picture`, [`crate::design`]): Core Animation's
//! `renderInContext:` of the content's layers at any scale and of any part,
//! into raw sRGB pixels. It needs neither the Screen Recording permission
//! nor the window shown, and the window shows nothing different meanwhile:
//! what the picture changes in the layers is put back in the same Core
//! Animation transaction, so the window server never sees it.
//!
//! Two things are drawn for the picture as the screen shows them, not as
//! an in-process rendering would:
//!
//! - **Outer box shadows** (`crate::visual`'s casters): an in-process
//!   rendering draws no shadow from a `shadowPath`, and cuts a masked
//!   layer off at its bounds. Each shadow holder's shadows are drawn here
//!   into an image from its casters' paths, offsets, blur and colours
//!   (the box left out where the holder's mask leaves it out), shown in
//!   their place while the picture is taken.
//! - **Animations that loop with no end** (a spinner) stand at the start of
//!   the run they are in ([`nsh_core::animation::Animator::at_loop_starts`]).

use std::rc::Rc;

use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_core_foundation::{CFRetained, CGFloat, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGColor, CGContext, CGImage, CGPath};
use objc2_foundation::ns_string;
use objc2_quartz_core::{CALayer, CAShapeLayer, CATransaction};

use crate::surface::Surface;

/// The name `crate::visual` gives the layer holding a view's outer box
/// shadows' casters.
pub(crate) const SHADOWS: &str = "nsh.shadows";

/// How a CSS-like blur is drawn by Core Graphics for Core Animation's
/// `shadowRadius` (close to the Gaussian's deviation): its blur is twice it.
const BLUR_PER_RADIUS: CGFloat = 2.0;

/// A picture: premultiplied RGBA, 8 bits a channel, sRGB, rows from the top,
/// no padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw {
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub pixels: Vec<u8>,
}

/// Why there is no picture: a code of `nsh_core::runtime::design::codes`
/// and a message.
pub(crate) type Failed = (&'static str, String);

/// `content` (a window's content view, laid out) drawn at `scale` pixels a
/// point: all of it, or `rect` (its points from its top left), which must
/// lie within it. `surface`, the surface whose views it shows, stands its
/// endless animations at their first frame for the picture.
pub(crate) fn draw(
    content: &NSView,
    scale: f64,
    rect: Option<CGRect>,
    surface: Option<&Rc<Surface>>,
) -> Result<Raw, Failed> {
    use nsh_core::runtime::design::codes;
    let bounds = content.bounds();
    let (width, height) = (bounds.size.width, bounds.size.height);
    let rect = rect.unwrap_or(CGRect::new(CGPoint::ZERO, CGSize::new(width, height)));
    // A hair over, from points that went through `f32`.
    const SLACK: CGFloat = 0.01;
    if rect.origin.x < -SLACK
        || rect.origin.y < -SLACK
        || rect.origin.x + rect.size.width > width + SLACK
        || rect.origin.y + rect.size.height > height + SLACK
        || rect.size.width <= 0.0
        || rect.size.height <= 0.0
    {
        return Err((
            codes::RECT_OUTSIDE,
            format!(
                "the rect {:?} is not within the window's content ({width} x {height} points)",
                (
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height
                )
            ),
        ));
    }
    let pw = (rect.size.width * scale).round().max(1.0) as usize;
    let ph = (rect.size.height * scale).round().max(1.0) as usize;
    let layer = content.layer().ok_or((
        codes::NO_WINDOW,
        "the window's content has no layer".to_owned(),
    ))?;
    content.layoutSubtreeIfNeeded();
    let flipped = content.isFlipped();
    let mut pixels = vec![0u8; pw * ph * 4];
    // One transaction: what the picture changes is put back before it
    // commits.
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    crate::synth::order_by_z(&layer);
    crate::shader::display_tree(&layer);
    if let Some(s) = surface {
        let now = objc2_quartz_core::CACurrentMediaTime();
        let mut still = s.anim.borrow().at_loop_starts(now);
        still.write(&s.mounter.borrow());
        crate::shader::display_tree(&layer);
    }
    let shadows = Shadows::stand_in(&layer, scale);
    let hidden = content
        .window()
        .map(|w| crate::shader::hide_masked(&w))
        .unwrap_or_default();
    render(&layer, &mut pixels, (pw, ph), scale, flipped, rect, height);
    hidden.iter().for_each(|c| c.setHidden(false));
    shadows.put_back();
    // The animations' values now, as the window shows them.
    if let Some(s) = surface {
        s.anim.borrow_mut().write(&s.mounter.borrow());
    }
    CATransaction::commit();
    Ok(Raw {
        width: pw as u32,
        height: ph as u32,
        pixels,
    })
}

/// Renders `layer` (of a view `flipped` or not, `height` points tall) into
/// `pixels`, `rect` of it at `scale`, top row first.
fn render(
    layer: &CALayer,
    pixels: &mut [u8],
    (pw, ph): (usize, usize),
    scale: f64,
    flipped: bool,
    rect: CGRect,
    height: CGFloat,
) {
    let Some(context) = rgba_context(pixels, pw, ph) else {
        return;
    };
    let c = Some(&*context);
    if flipped {
        CGContext::translate_ctm(c, 0.0, ph as CGFloat);
        CGContext::scale_ctm(c, scale, -scale);
        CGContext::translate_ctm(c, -rect.origin.x, -rect.origin.y);
    } else {
        CGContext::scale_ctm(c, scale, scale);
        CGContext::translate_ctm(
            c,
            -rect.origin.x,
            -(height - rect.origin.y - rect.size.height),
        );
    }
    layer.renderInContext(&context);
}

/// An sRGB RGBA context over `pixels` (`pw * ph * 4` bytes, rows from the
/// top), which must outlive it.
fn rgba_context(pixels: &mut [u8], pw: usize, ph: usize) -> Option<CFRetained<CGContext>> {
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGColorSpace, CGImageAlphaInfo, CGImageByteOrderInfo,
        kCGColorSpaceSRGB,
    };
    debug_assert_eq!(pixels.len(), pw * ph * 4);
    // SAFETY: a constant of CoreGraphics.
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))?;
    // SAFETY: `pixels` holds `pw * ph` RGBA pixels and outlives the
    // context, which the callers drop first.
    unsafe {
        CGBitmapContextCreate(
            pixels.as_mut_ptr().cast(),
            pw,
            ph,
            8,
            pw * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Big.0,
        )
    }
}

/// `image` as sRGB RGBA pixels (colour matched), for comparing with a
/// picture.
pub(crate) fn rgba(image: &CGImage) -> Raw {
    let (pw, ph) = (CGImage::width(Some(image)), CGImage::height(Some(image)));
    let mut pixels = vec![0u8; pw * ph * 4];
    if let Some(context) = rgba_context(&mut pixels, pw, ph) {
        CGContext::draw_image(
            Some(&context),
            CGRect::new(CGPoint::ZERO, CGSize::new(pw as CGFloat, ph as CGFloat)),
            Some(image),
        );
    }
    Raw {
        width: pw as u32,
        height: ph as u32,
        pixels,
    }
}

/// The shadow holders of a layer tree, standing in for the picture: their
/// casters hidden and their masks lifted, an image of their shadows in
/// their place.
struct Shadows(Vec<StandIn>);

struct StandIn {
    holder: Retained<CALayer>,
    mask: Option<Retained<CALayer>>,
    casters: Vec<Retained<CALayer>>,
    image: Retained<CALayer>,
}

impl Shadows {
    fn stand_in(layer: &CALayer, scale: f64) -> Shadows {
        let mut found = Vec::new();
        holders(layer, &mut found);
        Shadows(
            found
                .into_iter()
                .filter_map(|holder| StandIn::new(holder, scale))
                .collect(),
        )
    }

    fn put_back(self) {
        for s in self.0 {
            s.image.removeFromSuperlayer();
            for c in &s.casters {
                c.setHidden(false);
            }
            if let Some(mask) = &s.mask {
                // SAFETY: the holder's own mask, given back.
                unsafe { s.holder.setMask(Some(mask)) };
            }
        }
    }
}

/// The visible shadow holders under `layer`.
fn holders(layer: &CALayer, out: &mut Vec<Retained<CALayer>>) {
    if layer.isHidden() {
        return;
    }
    // SAFETY: a getter; the sublayers are retained while they are read.
    let Some(sublayers) = (unsafe { layer.sublayers() }) else {
        return;
    };
    for sublayer in sublayers.iter() {
        if sublayer.name().is_some_and(|n| n.to_string() == SHADOWS) {
            if !sublayer.isHidden() {
                out.push(sublayer.clone());
            }
        } else {
            holders(&sublayer, out);
        }
    }
}

/// A point of `layer`'s space in its superlayer's.
fn in_parent(layer: &CALayer) -> (CGFloat, CGFloat) {
    let (frame, bounds) = (layer.frame(), layer.bounds());
    (
        frame.origin.x - bounds.origin.x,
        frame.origin.y - bounds.origin.y,
    )
}

impl StandIn {
    fn new(holder: Retained<CALayer>, scale: f64) -> Option<StandIn> {
        // SAFETY: a getter; the sublayers are retained while they are read.
        let sublayers = unsafe { holder.sublayers() }?;
        let casters: Vec<Retained<CALayer>> = sublayers
            .iter()
            .filter(|l| !l.isHidden() && l.shadowOpacity() > 0.0 && l.shadowPath().is_some())
            .collect();
        if casters.is_empty() {
            return None;
        }
        // What the shadows cover, in the holder's space.
        let mut region: Option<CGRect> = None;
        for c in &casters {
            let path = c.shadowPath()?;
            let b = CGPath::bounding_box(Some(&path));
            let (dx, dy) = in_parent(c);
            let offset = c.shadowOffset();
            let reach = c.shadowRadius() * 3.0 + 2.0;
            let r = CGRect::new(
                CGPoint::new(
                    b.origin.x + dx + offset.width - reach,
                    b.origin.y + dy + offset.height - reach,
                ),
                CGSize::new(b.size.width + 2.0 * reach, b.size.height + 2.0 * reach),
            );
            region = Some(region.map_or(r, |u| union(u, r)));
        }
        let region = region?;
        let pw = (region.size.width * scale).ceil().clamp(1.0, 8192.0) as usize;
        let ph = (region.size.height * scale).ceil().clamp(1.0, 8192.0) as usize;
        // The image's top row is at the holder's top as it shows: its
        // smallest y where its contents are flipped (y grows down).
        let down = holder.contentsAreFlipped();
        let mask = holder.mask();
        let image = shadow_image(&casters, mask.as_deref(), region, (pw, ph), down)?;
        let stand_in = CALayer::new();
        stand_in.setFrame(region);
        stand_in.setContentsScale(scale);
        // SAFETY: a CGImage is what a layer's contents take.
        unsafe { stand_in.setContents(Some(as_object(&image))) };
        stand_in.setName(Some(ns_string!("nsh.shadows.picture")));
        for c in &casters {
            c.setHidden(true);
        }
        if mask.is_some() {
            // SAFETY: taken off for the picture, given back by `put_back`.
            unsafe { holder.setMask(None) };
        }
        holder.addSublayer(&stand_in);
        Some(StandIn {
            holder,
            mask,
            casters,
            image: stand_in,
        })
    }
}

fn as_object(image: &CGImage) -> &objc2::runtime::AnyObject {
    // SAFETY: a CGImage is a CoreFoundation object, toll-free an ObjC one.
    unsafe { &*(image as *const CGImage).cast::<objc2::runtime::AnyObject>() }
}

fn union(a: CGRect, b: CGRect) -> CGRect {
    let x0 = a.origin.x.min(b.origin.x);
    let y0 = a.origin.y.min(b.origin.y);
    let x1 = (a.origin.x + a.size.width).max(b.origin.x + b.size.width);
    let y1 = (a.origin.y + a.size.height).max(b.origin.y + b.size.height);
    CGRect::new(CGPoint::new(x0, y0), CGSize::new(x1 - x0, y1 - y0))
}

/// The shadows of `casters` (sublayers of `holder`) over `region` of the
/// holder's space, `pw` x `ph` pixels, top row at the region's smallest y
/// when `down`: each caster's `shadowPath` shadowed by its offset, radius,
/// colour and opacity, without the shape itself, and nothing where `mask`
/// (the holder's) hides (even-odd).
fn shadow_image(
    casters: &[Retained<CALayer>],
    mask: Option<&CALayer>,
    region: CGRect,
    (pw, ph): (usize, usize),
    down: bool,
) -> Option<CFRetained<CGImage>> {
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGColorSpace, CGImageAlphaInfo, CGImageByteOrderInfo,
        kCGColorSpaceSRGB,
    };
    let scale = pw as CGFloat / region.size.width;
    // SAFETY: a constant of CoreGraphics.
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))?;
    // SAFETY: CoreGraphics allocates the pixels (null data) and keeps them
    // for the image made from them.
    let context = unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            pw,
            ph,
            8,
            pw * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Big.0,
        )
    }?;
    let c = Some(&*context);
    // Holder points to pixels: y down (as the image is read) or up.
    let y_sign: CGFloat = if down {
        CGContext::translate_ctm(c, 0.0, ph as CGFloat);
        CGContext::scale_ctm(c, scale, -scale);
        -1.0
    } else {
        CGContext::scale_ctm(c, scale, scale);
        1.0
    };
    CGContext::translate_ctm(c, -region.origin.x, -region.origin.y);
    if let Some(mask) = mask.and_then(|m| {
        let path = m.downcast_ref::<CAShapeLayer>()?.path()?;
        Some((m, path))
    }) {
        let (m, path) = mask;
        let (dx, dy) = in_parent(m);
        CGContext::translate_ctm(c, dx, dy);
        CGContext::add_path(c, Some(&path));
        CGContext::translate_ctm(c, -dx, -dy);
        CGContext::eo_clip(c);
    }
    // The shape is drawn this far to the left, out of the image; its shadow
    // is moved back by as much.
    let far = region.size.width + 1000.0;
    for caster in casters {
        let Some(path) = caster.shadowPath() else {
            continue;
        };
        let opacity = caster.shadowOpacity() as CGFloat;
        let Some(color) = caster
            .shadowColor()
            .and_then(|c| CGColor::new_copy_with_alpha(Some(&c), opacity))
        else {
            continue;
        };
        let offset = caster.shadowOffset();
        let (dx, dy) = in_parent(caster);
        CGContext::save_g_state(c);
        // Shadow offsets and blur are in pixels, not the drawing's points.
        CGContext::set_shadow_with_color(
            c,
            CGSize::new((offset.width + far) * scale, offset.height * scale * y_sign),
            caster.shadowRadius() * BLUR_PER_RADIUS * scale,
            Some(&color),
        );
        CGContext::translate_ctm(c, dx - far, dy);
        CGContext::add_path(c, Some(&path));
        CGContext::fill_path(c);
        CGContext::restore_g_state(c);
    }
    objc2_core_graphics::CGBitmapContextCreateImage(Some(&context))
}
