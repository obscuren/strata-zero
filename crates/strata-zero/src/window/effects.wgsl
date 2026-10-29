// The window's screen effects. `fs_glow` draws the frame's bright colours into a small
// texture; `fs` draws the frame to the screen with that glow blurred on top, scanlines,
// a vignette and a curve, each as strong as `Params` says (zero is off).

struct Params {
    // The frame, in pixels.
    size: vec2<f32>,
    // The window's scale factor: 2 on a Retina screen.
    scale: f32,
    glow: f32,
    scan: f32,
    vignette: f32,
    curve: f32,
    // How many frame pixels one glow texel covers, across.
    down: f32,
};

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var glow_tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var<uniform> p: Params;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// One triangle covering the target, uv (0, 0) at the top-left.
@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    let xy = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.pos = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(xy.x, 1.0 - xy.y);
    return o;
}

// How much of a colour glows: keyed on its brightest channel, so magenta and cyan neon
// glow as much as white text does.
fn bright(c: vec3<f32>) -> vec3<f32> {
    let m = max(c.r, max(c.g, c.b));
    return c * smoothstep(0.3, 0.9, m);
}

// The bright pass, downsampled: a 4×4 grid of bilinear taps over the frame pixels this
// glow texel covers and a little beyond, which blurs it once already.
@fragment
fn fs_glow(in: VsOut) -> @location(0) vec4<f32> {
    let texel = 1.0 / p.size;
    var acc = vec3<f32>(0.0);
    for (var y = 0; y < 4; y++) {
        for (var x = 0; x < 4; x++) {
            let off = (vec2<f32>(f32(x), f32(y)) - 1.5) * p.down * 0.5 * texel;
            acc += bright(textureSampleLevel(frame, samp, in.uv + off, 0.0).rgb);
        }
    }
    return vec4<f32>(acc / 16.0, 1.0);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    var uv = in.uv;
    if (p.curve > 0.0) {
        // Barrel distortion scaled so the middle of each edge stays on the edge: no
        // column is lost, only the corners round off.
        let c = uv * 2.0 - 1.0;
        uv = c * (1.0 + p.curve * dot(c, c)) / (1.0 + p.curve) * 0.5 + 0.5;
        if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
    }
    var col = textureSampleLevel(frame, samp, uv, 0.0).rgb;
    if (p.glow > 0.0) {
        // A tent over 3×3 glow texels, spread a texel and a half apart.
        let gt = 1.0 / vec2<f32>(textureDimensions(glow_tex));
        var w = array<f32, 3>(0.25, 0.5, 0.25);
        var g = vec3<f32>(0.0);
        for (var y = 0; y < 3; y++) {
            for (var x = 0; x < 3; x++) {
                let off = vec2<f32>(f32(x - 1), f32(y - 1)) * 1.5 * gt;
                g += textureSampleLevel(glow_tex, samp, uv + off, 0.0).rgb * w[x] * w[y];
            }
        }
        col += g * p.glow;
    }
    if (p.scan > 0.0) {
        // A dark line every two logical pixels, following the curve. The half-pixel
        // shift puts pixel centres on the wave's crests, not its zero crossings.
        let row = uv.y * p.size.y / p.scale - 0.5;
        col *= 1.0 - p.scan * (0.5 + 0.5 * cos(row * 3.14159265));
    }
    if (p.vignette > 0.0) {
        let d = in.uv * 2.0 - 1.0;
        col *= 1.0 - p.vignette * smoothstep(0.6, 1.5, length(d));
    }
    return vec4<f32>(col, 1.0);
}
