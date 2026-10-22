package com.longbridge.nsh;

import android.annotation.SuppressLint;
import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.BitmapShader;
import android.graphics.Canvas;
import android.graphics.Color;
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

    /** `nsh_core::shader::PROGRESS`: the component an animated `progress` writes. */
    private static final int PROGRESS = 0xffff;
    private static final int CONTENT = 0, BACKDROP = 1;

    private final boolean effect;
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

    ShaderBox(Context context, Mounter mounter, int id, boolean effect) {
        super(context, mounter, id);
        this.effect = effect;
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
        boolean behind = Build.VERSION.SDK_INT >= 33 && samples(BACKDROP) && !(effect && samples(CONTENT));
        if (behind && backdrop == null) {
            backdrop = new Backdrop(this);
        } else if (!behind && backdrop != null) {
            backdrop.release();
            backdrop = null;
        }
        if (effect && samples(CONTENT) && samples(BACKDROP) && !warned) {
            warned = true;
            android.util.Log.w("nsh", "<ShaderEffect>: Android gives a shader one live input; the backdrop reads as transparent");
        }
        invalidate();
    }

    private boolean samples(int input) {
        return (inputs & (1 << input)) != 0;
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

    @Override
    protected void dispatchDraw(Canvas canvas) {
        if (!effect || !runs(canvas)) {
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
        node.setRenderEffect(RenderEffect.createRuntimeShaderEffect(shader, inputName(CONTENT)));
        canvas.save();
        canvas.translate(-m, -m);
        canvas.drawRenderNode(node);
        canvas.restore();
        if (time && isShown()) postInvalidateOnAnimation();
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
        float width = getWidth() / density, height = getHeight() / density;
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
            if (input == CONTENT || input == BACKDROP) {
                frame[at] = Float.floatToRawIntBits(x);
                frame[at + 1] = Float.floatToRawIntBits(y);
                frame[at + 2] = Float.floatToRawIntBits(w);
                frame[at + 3] = Float.floatToRawIntBits(h);
                frame[at + 4] = Float.floatToRawIntBits(pw);
                frame[at + 5] = Float.floatToRawIntBits(ph);
                // The render effect binds the live one; the other reads transparent.
                boolean live = node != null && (effect && backdrop == null ? input == CONTENT : input == BACKDROP);
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
