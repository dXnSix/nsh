package com.longbridge.nsh;

import android.annotation.SuppressLint;
import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.BitmapShader;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.ColorSpace;
import android.graphics.HardwareRenderer;
import android.graphics.PixelFormat;
import android.hardware.HardwareBuffer;
import android.media.Image;
import android.media.ImageReader;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.Rect;
import android.graphics.RecordingCanvas;
import android.graphics.RenderEffect;
import android.graphics.RenderNode;
import android.graphics.RuntimeShader;
import android.graphics.Shader;
import android.os.Build;
import android.os.SystemClock;
import android.view.View;

/**
 * `<shader>` and `<shadereffect>`: AGSL runtime shaders (`nsh_core::shader`,
 * Android 13 and later). Earlier versions draw the children only.
 *
 * A fill draws in {@link #onDraw}, over the background and under the
 * borders and children. A fill that samples what is behind it runs as the
 * render effect of a node drawing {@link Backdrop}'s recording, so it reads
 * the backdrop live. An effect ({@code effect}) records its children into a
 * render node (beyond its bounds by {@code maxSampleOffset}) whose render
 * effect is the shader, its content input that node: the children stay
 * views (touch, accessibility) and the shader reads them live. A render
 * effect gives a shader one live input: an effect that samples its content
 * and the backdrop reads the backdrop as transparent.
 *
 * A transition ({@code transition}) is an effect while it runs
 * ({@link #startTransition}): its children are its live `sample_to`, a
 * picture of them taken when it starts (rendered by a {@link
 * HardwareRenderer} into a hardware bitmap, which later changes to the
 * children leave as it is) its `sample_from`, and `nsh.progress` moves
 * from 0 to 1 on animation frames; then the children draw as they are.
 *
 * A screen effect ({@link #screen} 1, `<ScreenEffect>`) is a box of no
 * size whose shader runs as the render effect of the surface's root and of
 * each dialog's view ({@link Mounter#applyScreen}), every one of them its
 * live content, in points from the surface's top-left: `nsh.size` is the
 * surface's ({@link #screenEffect}). A screen transition ({@link #screen}
 * 2, `<ScreenTransition>`) makes an overlay of its shader over the surface
 * ({@link #overlay}, {@link Mounter#screenTransition}).
 *
 * The program (`encode.rs` `shader_program`) names each AGSL uniform and the
 * words of a buffer it takes: `nsh` (size, scale, time, progress), the frame
 * (the area drawn and the inputs' rects and pixel sizes) or the app's
 * uniforms (`encode.rs` `shader_fill`, animated components written in by
 * {@link #setNumber} and {@link #setColor}).
 */
@SuppressLint("ViewConstructor")
final class ShaderBox extends Box {
    /** A program: its source and how to fill its uniforms and inputs. */
    static final class Program {
        String source;
        String[] uniforms;
        int[] buffers;
        boolean[] ints;
        int[][] words;
        String[] shaders;
        int[] inputs;
    }

    /**
     * A page's or a dialog's transition through a shader
     * (`nsh_core::NavShader`): its program, inputs and uniforms, and how
     * long it runs.
     */
    static final class Nav {
        Program program;
        int inputs;
        int[] uniforms;
        float ms;
    }

    /** `nsh_core::shader::PROGRESS`: the component an animated `progress` writes. */
    private static final int PROGRESS = 0xffff;
    private static final int CONTENT = 0, BACKDROP = 1, FROM = 2, TO = 3;

    private final boolean effect;
    private final boolean transition;
    /** A running transition's `sample_from`, start (uptime) and length. */
    private Bitmap from;
    /** A page's or a dialog's transition: a picture of its upper side (`sample_to`). */
    private Bitmap toPicture;
    private long transitionStart = -1;
    private float transitionMs;
    private Program program;
    private RuntimeShader shader;
    private final Paint paint = new Paint();
    private int[] uniforms = new int[0];
    private boolean time;
    private int inputs;
    private float progress, maxOffset;
    private long started = -1;
    /** Images by slot, once loaded; their URIs and the words of their size. */
    final Bitmap[] images = new Bitmap[8];
    String[] uris = new String[0];
    int[] sizeWords = new int[0];
    private final Path shapePath = new Path();
    /** What is behind the view, while the shader samples it. */
    private Backdrop backdrop;
    /** The node the shader runs on as its render effect (an effect's children, or the backdrop). */
    private RenderNode node;
    private boolean warned;
    /** A screen effect's (1) or a screen transition's (2) box; 0 otherwise. */
    int screen;
    /** The surface's size in points while a screen effect prepares its uniforms (`nsh.size`); -1 otherwise. */
    private float screenWidth = -1, screenHeight = -1;

    ShaderBox(Context context, Mounter mounter, int id, boolean effect, boolean transition) {
        super(context, mounter, id);
        this.effect = effect;
        this.transition = transition;
        setWillNotDraw(false);
        // A fill goes over the background and under the borders.
        if (!effect) decor.bordersAbove = true;
    }

    /** What it draws: `program` (null: nothing) with `uniforms`. */
    void setFill(Program program, boolean time, int inputs, float progress, float maxOffset,
            int[] uniforms, String[] uris, int[] sizeWords) {
        if (program != this.program) {
            this.program = program;
            shader = null;
            if (program != null && Build.VERSION.SDK_INT >= 33) {
                try {
                    shader = new RuntimeShader(program.source);
                } catch (IllegalArgumentException e) {
                    android.util.Log.e("nsh", "<Shader>: the runtime shader did not compile: " + e.getMessage());
                }
            }
        }
        this.time = time;
        this.inputs = program != null ? inputs : 0;
        this.progress = progress;
        this.maxOffset = maxOffset;
        this.uniforms = uniforms;
        for (int slot = 0; slot < images.length; slot++) {
            String before = slot < this.uris.length ? this.uris[slot] : null;
            String now = slot < uris.length ? uris[slot] : null;
            if (now == null || !now.equals(before)) images[slot] = null;
        }
        this.uris = uris;
        this.sizeWords = sizeWords;
        if (program == null) endTransition();
        boolean behind = Build.VERSION.SDK_INT >= 33 && samples(BACKDROP) && !(effect && samples(children()));
        if (behind && backdrop == null) {
            backdrop = new Backdrop(this);
        } else if (!behind && backdrop != null) {
            backdrop.release();
            backdrop = null;
        }
        if (effect && samples(children()) && samples(BACKDROP) && !warned) {
            warned = true;
            android.util.Log.w("nsh", "<ShaderEffect>: Android gives a shader one live input; the backdrop reads as transparent");
        }
        invalidate();
    }

    private boolean samples(int input) {
        return (inputs & (1 << input)) != 0;
    }

    /** The input its children are: an effect's content, a transition's `sample_to`. */
    private int children() {
        return transition ? TO : CONTENT;
    }

    private boolean running() {
        return transitionStart >= 0;
    }

    /**
     * A transition starts, before the rest of its batch: what shows now
     * (the children, or the transition as it is drawn) becomes its
     * `sample_from`, and `nsh.progress` goes from 0 to 1 over `ms`.
     */
    void startTransition(float ms) {
        if (!transition || shader == null || Build.VERSION.SDK_INT < 33 || getWidth() <= 0 || getHeight() <= 0) return;
        int m = Math.round(maxOffset * mounter.density);
        int w = getWidth() + 2 * m, h = getHeight() + 2 * m;
        RenderNode picture = new RenderNode("nsh-transition-from");
        picture.setPosition(0, 0, w, h);
        RecordingCanvas recording = picture.beginRecording(w, h);
        if (running() && node != null) {
            recording.drawRenderNode(node);
        } else {
            recording.translate(m, m);
            super.dispatchDraw(recording);
        }
        picture.endRecording();
        Bitmap bitmap = render(picture, w, h);
        if (bitmap == null) return;
        if (from != null) from.recycle();
        from = bitmap;
        transitionStart = SystemClock.uptimeMillis();
        transitionMs = Math.max(1, ms);
        invalidate();
    }

    /**
     * A page's or a dialog's transition through a shader, this view over
     * them: pictures of its lower side (`sample_from`) and its upper side
     * (`sample_to`).
     */
    void setSides(Bitmap lower, Bitmap upper) {
        from = lower;
        toPicture = upper;
        invalidate();
    }

    /**
     * `views` drawn one over the other as they are now, rendered once into
     * a hardware bitmap the size of the first (null if it cannot be).
     */
    static Bitmap picture(View... views) {
        if (Build.VERSION.SDK_INT < 33 || views.length == 0) return null;
        int w = views[0].getWidth(), h = views[0].getHeight();
        if (w <= 0 || h <= 0) return null;
        RenderNode node = new RenderNode("nsh-nav-picture");
        node.setPosition(0, 0, w, h);
        RecordingCanvas recording = node.beginRecording(w, h);
        // Each where it is on the screen, from the first's top-left (a
        // dialog's window covers the screen, the app's view may not).
        int[] base = new int[2], at = new int[2];
        views[0].getLocationOnScreen(base);
        for (View v : views) {
            if (v == null || v.getWidth() <= 0) continue;
            v.getLocationOnScreen(at);
            int saved = recording.save();
            recording.translate(at[0] - base[0], at[1] - base[1]);
            v.draw(recording);
            recording.restoreToCount(saved);
        }
        node.endRecording();
        return render(node, w, h);
    }

    /** `node` rendered once into a hardware bitmap of `w` × `h` pixels (null if it could not be). */
    private static Bitmap render(RenderNode node, int w, int h) {
        ImageReader reader = ImageReader.newInstance(w, h, PixelFormat.RGBA_8888, 1,
                HardwareBuffer.USAGE_GPU_SAMPLED_IMAGE | HardwareBuffer.USAGE_GPU_COLOR_OUTPUT);
        HardwareRenderer renderer = new HardwareRenderer();
        try {
            renderer.setSurface(reader.getSurface());
            renderer.setContentRoot(node);
            renderer.createRenderRequest().setWaitForPresent(true).syncAndDraw();
            Image image = reader.acquireNextImage();
            if (image == null) return null;
            HardwareBuffer buffer = image.getHardwareBuffer();
            Bitmap bitmap = buffer != null ? Bitmap.wrapHardwareBuffer(buffer, ColorSpace.get(ColorSpace.Named.SRGB)) : null;
            if (buffer != null) buffer.close();
            image.close();
            return bitmap;
        } catch (RuntimeException e) {
            android.util.Log.w("nsh", "<ShaderTransition>: no picture of the children: " + e.getMessage());
            return null;
        } finally {
            renderer.destroy();
            reader.close();
        }
    }

    /** Whether its shader runs (Android 13 and later, a program that compiled). */
    boolean screenRuns() {
        return shader != null && Build.VERSION.SDK_INT >= 33;
    }

    /** Whether its shader reads `nsh.time`. */
    boolean readsTime() {
        return time;
    }

    /**
     * A screen effect over a view covering `x, y, w, h` (points from the
     * surface's top-left) at `pw` × `ph` pixels, the view's content its
     * live `sample_content`, the surface `surfaceW` × `surfaceH` points.
     */
    RenderEffect screenEffect(float x, float y, float w, float h, int pw, int ph, float surfaceW, float surfaceH) {
        screenWidth = surfaceW;
        screenHeight = surfaceH;
        try {
            prepare(x, y, w, h, pw, ph);
        } finally {
            screenWidth = -1;
            screenHeight = -1;
        }
        return RenderEffect.createRuntimeShaderEffect(shader, inputName(CONTENT));
    }

    /** A screen transition's overlay: a box drawing its shader, its sides set by {@link #setSides}. */
    ShaderBox overlay() {
        ShaderBox o = new ShaderBox(getContext(), mounter, -1, false, false);
        o.setFill(program, time, inputs, 0, 0, uniforms.clone(), uris, sizeWords);
        System.arraycopy(images, 0, o.images, 0, images.length);
        return o;
    }

    @Override
    public void invalidate() {
        super.invalidate();
        // A screen effect draws over other views: they take it again.
        if (screen == 1 && mounter != null) mounter.screenDirty();
    }

    /** The image of `slot` loaded (it was `uri`'s); `width` × `height` points. */
    void setImage(int slot, String uri, Bitmap bitmap, float width, float height) {
        if (slot >= uris.length || !uri.equals(uris[slot])) return;
        images[slot] = bitmap;
        int at = sizeWords[slot];
        // The size the source said first, else the image's.
        if (at + 1 < uniforms.length && uniforms[at] == 0 && uniforms[at + 1] == 0) {
            uniforms[at] = Float.floatToRawIntBits(width);
            uniforms[at + 1] = Float.floatToRawIntBits(height);
        }
        invalidate();
    }

    /** An animated number at word `k` of the uniforms, or the progress. */
    void setNumber(int k, float value) {
        if (k == PROGRESS) {
            if (progress == value) return;
            progress = value;
            invalidate();
            return;
        }
        if (k >= uniforms.length || uniforms[k] == Float.floatToRawIntBits(value)) return;
        uniforms[k] = Float.floatToRawIntBits(value);
        invalidate();
    }

    /** An animated color (ARGB) at words `k`..`k + 3`, premultiplied. */
    void setColor(int k, int argb) {
        if (k + 3 >= uniforms.length) return;
        float a = Color.alpha(argb) / 255f;
        float[] c = {Color.red(argb) / 255f * a, Color.green(argb) / 255f * a, Color.blue(argb) / 255f * a, a};
        for (int i = 0; i < 4; i++) uniforms[k + i] = Float.floatToRawIntBits(c[i]);
        invalidate();
    }

    private static Shader transparent;

    /** What an input without an image samples: transparent. */
    private static Shader transparent() {
        if (transparent == null) {
            transparent = new BitmapShader(Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888),
                    Shader.TileMode.CLAMP, Shader.TileMode.CLAMP);
        }
        return transparent;
    }

    /** Software canvases (pictures drawn into a bitmap) take no runtime shader. */
    private boolean runs(Canvas canvas) {
        return shader != null && Build.VERSION.SDK_INT >= 33 && getWidth() > 0 && getHeight() > 0
                && canvas.isHardwareAccelerated();
    }

    @Override
    protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        if (!effect && runs(canvas)) {
            if (backdrop != null) drawOverBackdrop(canvas);
            else drawFill(canvas);
        }
        if (!effect) decor.drawBorders(canvas);
    }

    private void endTransition() {
        transitionStart = -1;
        if (from != null) {
            from.recycle();
            from = null;
        }
    }

    @Override
    protected void dispatchDraw(Canvas canvas) {
        if (transition && running()) {
            float t = (SystemClock.uptimeMillis() - transitionStart) / transitionMs;
            if (t >= 1) endTransition();
            else progress = t;
        }
        // An idle transition draws its children as they are.
        if (!effect || !runs(canvas) || (transition && !running())) {
            super.dispatchDraw(canvas);
            return;
        }
        if (backdrop != null) {
            // It shows what is behind it through the shader, not its children.
            drawOverBackdrop(canvas);
            return;
        }
        drawEffect(canvas);
    }

    /** The children through the shader, `maxOffset` points beyond the view on every side. */
    private void drawEffect(Canvas canvas) {
        float density = mounter.density;
        int m = Math.round(maxOffset * density);
        int w = getWidth() + 2 * m, h = getHeight() + 2 * m;
        if (node == null) node = new RenderNode("nsh-shader-effect");
        node.setPosition(0, 0, w, h);
        RecordingCanvas recording = node.beginRecording(w, h);
        recording.translate(m, m);
        super.dispatchDraw(recording);
        node.endRecording();
        float mp = m / density;
        prepare(-mp, -mp, w / density, h / density, w, h);
        node.setRenderEffect(RenderEffect.createRuntimeShaderEffect(shader, inputName(children())));
        canvas.save();
        canvas.translate(-m, -m);
        canvas.drawRenderNode(node);
        canvas.restore();
        if ((time || running()) && isShown()) postInvalidateOnAnimation();
        else if (running()) invalidate();
    }

    /** What is behind the view through the shader, in the view's shape. */
    private void drawOverBackdrop(Canvas canvas) {
        int w = getWidth(), h = getHeight();
        if (node == null) node = new RenderNode("nsh-shader-backdrop");
        node.setPosition(0, 0, w, h);
        RecordingCanvas recording = node.beginRecording(w, h);
        backdrop.draw(recording);
        node.endRecording();
        prepare(0, 0, w / mounter.density, h / mounter.density, w, h);
        node.setRenderEffect(RenderEffect.createRuntimeShaderEffect(shader, inputName(BACKDROP)));
        canvas.save();
        if (decor.rounded()) canvas.clipPath(decor.shape(new Rect(0, 0, w, h), 0, shapePath));
        canvas.drawRenderNode(node);
        canvas.restore();
        // The backdrop changes with what is behind: the node is recorded again each frame.
        if (isShown()) postInvalidateOnAnimation();
    }

    private void drawFill(Canvas canvas) {
        prepare(0, 0, getWidth() / mounter.density, getHeight() / mounter.density, getWidth(), getHeight());
        paint.setShader(shader);
        // The fill follows the view's corners, as a background does.
        if (decor.rounded()) {
            canvas.drawPath(decor.shape(new Rect(0, 0, getWidth(), getHeight()), 0, shapePath), paint);
        } else {
            canvas.drawRect(0, 0, getWidth(), getHeight(), paint);
        }
        // A shader that reads `nsh.time` draws on every frame while it shows.
        if (time && isShown()) postInvalidateOnAnimation();
    }

    /** The uniform shader that reads input `input` (the render effect's). */
    private String inputName(int input) {
        for (int i = 0; i < program.inputs.length; i++) if (program.inputs[i] == input) return program.shaders[i];
        return "";
    }

    /**
     * Sets every uniform for a drawing of the area `x, y, w, h` (points
     * from the view's top-left) at `pw` × `ph` pixels; images are input
     * shaders, the live input (content or backdrop) covers the area.
     */
    private void prepare(float x, float y, float w, float h, int pw, int ph) {
        float density = mounter.density;
        float width = screenWidth >= 0 ? screenWidth : getWidth() / density;
        float height = screenWidth >= 0 ? screenHeight : getHeight() / density;
        long now = SystemClock.uptimeMillis();
        if (started < 0) started = now;
        float seconds = (now - started) / 1000f;
        int[] nsh = {
            Float.floatToRawIntBits(width), Float.floatToRawIntBits(height), Float.floatToRawIntBits(density),
            Float.floatToRawIntBits(seconds), Float.floatToRawIntBits(progress), 0, 0, 0,
        };
        int[] frame = new int[8 + 8 * 12];
        frame[0] = Float.floatToRawIntBits(x);
        frame[1] = Float.floatToRawIntBits(y);
        frame[2] = Float.floatToRawIntBits(w);
        frame[3] = Float.floatToRawIntBits(h);
        for (int i = 0; i < program.inputs.length; i++) {
            int input = program.inputs[i];
            int at = 8 + 8 * input;
            if (input == FROM) {
                frame[at] = Float.floatToRawIntBits(x);
                frame[at + 1] = Float.floatToRawIntBits(y);
                frame[at + 2] = Float.floatToRawIntBits(w);
                frame[at + 3] = Float.floatToRawIntBits(h);
                frame[at + 4] = Float.floatToRawIntBits(from != null ? from.getWidth() : 1);
                frame[at + 5] = Float.floatToRawIntBits(from != null ? from.getHeight() : 1);
                shader.setInputShader(program.shaders[i], from != null
                        ? new BitmapShader(from, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP) : transparent());
                continue;
            }
            if (input == TO && toPicture != null) {
                frame[at] = Float.floatToRawIntBits(x);
                frame[at + 1] = Float.floatToRawIntBits(y);
                frame[at + 2] = Float.floatToRawIntBits(w);
                frame[at + 3] = Float.floatToRawIntBits(h);
                frame[at + 4] = Float.floatToRawIntBits(toPicture.getWidth());
                frame[at + 5] = Float.floatToRawIntBits(toPicture.getHeight());
                shader.setInputShader(program.shaders[i],
                        new BitmapShader(toPicture, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP));
                continue;
            }
            if (input == CONTENT || input == BACKDROP || input == TO) {
                frame[at] = Float.floatToRawIntBits(x);
                frame[at + 1] = Float.floatToRawIntBits(y);
                frame[at + 2] = Float.floatToRawIntBits(w);
                frame[at + 3] = Float.floatToRawIntBits(h);
                frame[at + 4] = Float.floatToRawIntBits(pw);
                frame[at + 5] = Float.floatToRawIntBits(ph);
                // The render effect binds the live one; the other reads transparent.
                boolean live = screenWidth >= 0 ? input == CONTENT
                        : node != null && (effect && backdrop == null ? input == children() : input == BACKDROP);
                if (!live) shader.setInputShader(program.shaders[i], transparent());
                continue;
            }
            Shader child = null;
            float iw = 1, ih = 1;
            if (input >= 4 && input - 4 < images.length && images[input - 4] != null) {
                Bitmap bitmap = images[input - 4];
                child = new BitmapShader(bitmap, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP);
                iw = bitmap.getWidth();
                ih = bitmap.getHeight();
            }
            if (child == null) child = transparent();
            frame[at + 2] = Float.floatToRawIntBits(width);
            frame[at + 3] = Float.floatToRawIntBits(height);
            frame[at + 4] = Float.floatToRawIntBits(iw);
            frame[at + 5] = Float.floatToRawIntBits(ih);
            shader.setInputShader(program.shaders[i], child);
        }
        for (int u = 0; u < program.uniforms.length; u++) {
            int[] source = program.buffers[u] == 0 ? nsh : program.buffers[u] == 1 ? frame : uniforms;
            int[] words = program.words[u];
            if (program.ints[u]) {
                int[] values = new int[words.length];
                for (int k = 0; k < words.length; k++) values[k] = words[k] < source.length ? source[words[k]] : 0;
                shader.setIntUniform(program.uniforms[u], values);
            } else {
                float[] values = new float[words.length];
                for (int k = 0; k < words.length; k++) {
                    values[k] = words[k] < source.length ? Float.intBitsToFloat(source[words[k]]) : 0;
                }
                shader.setFloatUniform(program.uniforms[u], values);
            }
        }
    }
}
