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
import android.graphics.RuntimeShader;
import android.graphics.Shader;
import android.os.Build;
import android.os.SystemClock;

/**
 * `<shader>`: a box an AGSL runtime shader fills (`nsh_core::shader`,
 * Android 13 and later), drawn in {@link #onDraw} over its background and
 * under its children. Earlier versions draw the children only.
 *
 * The program (`encode.rs` `shader_program`) names each AGSL uniform and the
 * words of a buffer it takes: `nsh` (size, scale, time, progress), the frame
 * (the inputs' rects and pixel sizes) or the app's uniforms (`encode.rs`
 * `shader_fill`, animated components written in by {@link #setNumber} and
 * {@link #setColor}).
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

    private Program program;
    private RuntimeShader shader;
    private final Paint paint = new Paint();
    private int[] uniforms = new int[0];
    private boolean time;
    private long started = -1;
    /** Images by slot, once loaded; their URIs and the words of their size. */
    final Bitmap[] images = new Bitmap[8];
    String[] uris = new String[0];
    int[] sizeWords = new int[0];
    private final Path shapePath = new Path();

    ShaderBox(Context context, Mounter mounter, int id) {
        super(context, mounter, id);
        setWillNotDraw(false);
        // The fill goes over the background and under the borders.
        decor.bordersAbove = true;
    }

    /** What it draws: `program` (null: nothing) with `uniforms`. */
    void setFill(Program program, boolean time, int[] uniforms, String[] uris, int[] sizeWords) {
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
        this.uniforms = uniforms;
        for (int slot = 0; slot < images.length; slot++) {
            String before = slot < this.uris.length ? this.uris[slot] : null;
            String now = slot < uris.length ? uris[slot] : null;
            if (now == null || !now.equals(before)) images[slot] = null;
        }
        this.uris = uris;
        this.sizeWords = sizeWords;
        invalidate();
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

    /** An animated number at word `k` of the uniforms. */
    void setNumber(int k, float value) {
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

    @Override
    protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        // Software canvases (pictures drawn into a bitmap) take no runtime shader.
        if (shader != null && Build.VERSION.SDK_INT >= 33 && getWidth() > 0 && getHeight() > 0
                && canvas.isHardwareAccelerated()) {
            drawShader(canvas);
        }
        decor.drawBorders(canvas);
    }

    private void drawShader(Canvas canvas) {
        float density = mounter.density;
        float width = getWidth() / density, height = getHeight() / density;
        long now = SystemClock.uptimeMillis();
        if (started < 0) started = now;
        float seconds = (now - started) / 1000f;
        int[] nsh = {
            Float.floatToRawIntBits(width), Float.floatToRawIntBits(height), Float.floatToRawIntBits(density),
            Float.floatToRawIntBits(seconds), 0, 0, 0, 0,
        };
        int[] frame = new int[8 + 8 * 12];
        frame[2] = Float.floatToRawIntBits(width);
        frame[3] = Float.floatToRawIntBits(height);
        for (int i = 0; i < program.inputs.length; i++) {
            int input = program.inputs[i];
            Shader child = null;
            float pw = 1, ph = 1;
            if (input >= 4 && input - 4 < images.length && images[input - 4] != null) {
                Bitmap bitmap = images[input - 4];
                child = new BitmapShader(bitmap, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP);
                pw = bitmap.getWidth();
                ph = bitmap.getHeight();
            }
            if (child == null) child = transparent();
            int at = 8 + 8 * input;
            frame[at + 2] = Float.floatToRawIntBits(width);
            frame[at + 3] = Float.floatToRawIntBits(height);
            frame[at + 4] = Float.floatToRawIntBits(pw);
            frame[at + 5] = Float.floatToRawIntBits(ph);
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
}
