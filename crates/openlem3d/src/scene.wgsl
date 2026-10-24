// Scene shaders: palette-textured unlit geometry, and the panned sky backdrop.

struct Uniforms {
    view_proj: mat4x4<f32>,
    // x: sky column (in sky texels) at the left edge of the screen,
    // y: horizon row in framebuffer pixels,
    // z: framebuffer pixels per original-screen pixel (height / 480),
    // w: time in seconds.
    sky: vec4<f32>,
    // Camera right vector (xyz); billboards are spanned by it and +Y.
    // w: 1 for an all-round tiled sky, 0 for a panorama above the horizon.
    right: vec4<f32>,
    // x: camera roll in radians (anticlockwise on screen), y: the
    // framebuffer x of the screen centre; the sky turns about (y, sky.y).
    // zw: screen pixels per rendered pixel across and down (1 unless the
    // scene is rendered small and stretched); the sky is laid out in
    // screen pixels.
    view: vec4<f32>,
    // The view's top-left corner in the framebuffer (the camera's viewport):
    // xy for the sky, 0 when the scene is rendered small; zw for the stretch.
    origin: vec4<f32>,
    // x: how much brighter than its palette colour a fully glowing texel is
    // drawn; 0 when glow is off. y: how far above white the screen shows
    // (an HDR target, linear with 1 at white), or 0.
    post: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
// The bloom chain's top level, for presenting the glowing scene.
@group(1) @binding(1) var bloom_tex: texture_2d<f32>;
@group(1) @binding(2) var bloom_samp: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // x: brightness, y: 1 if palette index 0 is transparent.
    @location(2) params: vec2<f32>,
    // Billboard corner offset from `pos`, in world units along the camera
    // right vector (x) and up (y). Zero for fixed geometry.
    @location(3) offset: vec2<f32>,
    // x: animation frame count (<= 1: static; negative: ping-pong over
    // that many frames), y: uv distance between frames.
    // A negative y marks a tiled surface (the sea): its frames are stacked
    // in the texture, the frame is chosen inside each repeat, and `offset`
    // is then a uv drift per second instead of a billboard offset.
    @location(4) anim: vec2<f32>,
    // Light from glowing things nearby, linear RGB (`light.rs`); used only
    // while glowing.
    @location(5) light: vec3<f32>,
};

/// Alpha steps from a plain texel to a fully glowing one (`glow.rs`).
const GLOW_STEPS: f32 = 120.0;

/// A texel's glow (0–1) from its alpha (`glow.rs`).
fn glow_of(a: f32) -> f32 {
    return clamp((1.0 - a) * 255.0 / GLOW_STEPS, 0.0, 1.0);
}

/// Animation frames per second of animated sprites (unverified).
const ANIM_FPS: f32 = 8.0;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) params: vec2<f32>,
    // For tiled surfaces: frame count and current frame; x = 0 otherwise.
    @location(2) tile: vec2<f32>,
    @location(3) light: vec3<f32>,
};

@vertex
fn vs(v: VIn) -> VOut {
    var o: VOut;
    var frame = 0.0;
    let step = floor(u.sky.w * ANIM_FPS);
    if (v.anim.x > 1.0) {
        frame = step % v.anim.x;
    } else if (v.anim.x < -1.0) {
        // Ping-pong: 0, 1, …, n − 1, n − 2, …, 1.
        let n = -v.anim.x;
        let s = step % (2.0 * n - 2.0);
        frame = select(2.0 * n - 2.0 - s, s, s < n);
    }
    o.params = v.params;
    // The lights flicker together, a little (ours).
    let t = u.sky.w;
    let flicker = 0.9 + 0.06 * sin(t * 7.3) + 0.04 * sin(t * 17.9 + 1.3);
    o.light = select(vec3(0.0), v.light * flicker, u.post.x > 0.0);
    if (v.anim.y < 0.0) {
        o.clip = u.view_proj * vec4(v.pos, 1.0);
        o.uv = v.uv + v.offset * u.sky.w;
        o.tile = vec2(max(v.anim.x, 1.0), frame);
        return o;
    }
    let world = v.pos + u.right.xyz * v.offset.x + vec3(0.0, v.offset.y, 0.0);
    o.clip = u.view_proj * vec4(world, 1.0);
    // A billboard is hidden only by what is in front of its middle (a
    // quarter unit above its anchor), all of it at that depth: a lemming
    // turning in a deflector's cell, half inside the block, is drawn whole,
    // as the original draws it (owner's observation).
    if (any(v.offset != vec2(0.0))) {
        let mid = u.view_proj * vec4(v.pos + vec3(0.0, 0.25, 0.0), 1.0);
        o.clip.z = mid.z / mid.w * o.clip.w;
    }
    o.uv = v.uv + vec2(0.0, frame * v.anim.y);
    o.tile = vec2(0.0, 0.0);
    return o;
}

@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> {
    var uv = v.uv;
    if (v.tile.x > 0.0) {
        uv = vec2(uv.x, (fract(uv.y) + v.tile.y) / v.tile.x);
    }
    let c = textureSample(tex, samp, uv);
    if (v.params.y > 0.5 && c.a < 0.5) {
        discard;
    }
    // Brightness darkens the palette colours as the original does, i.e. in
    // sRGB space; the sampled colour is linear, so the factor is linearised.
    // Glowing texels (alpha below 1) are lifted above white.
    let glow = glow_of(c.a);
    return vec4(c.rgb * (pow(v.params.x, 2.2) + v.light) * (1.0 + glow * u.post.x), 1.0);
}

// Sky: a full-screen triangle. Above the horizon the sky image is stretched
// so its full height spans the screen top to the horizon; horizontally each
// sky texel covers two original-screen pixels, and the image wraps.

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) i: u32) -> SkyOut {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    var o: SkyOut;
    o.clip = vec4(p * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

@fragment
fn fs_sky(v: SkyOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(tex));
    // Undo the camera roll: the point of the unrolled sky that lands here.
    // In framebuffer coordinates (y down) the anticlockwise roll's inverse is
    // the standard rotation matrix by the same angle.
    let pivot = vec2(u.view.y, u.sky.y);
    let d = (v.clip.xy - u.origin.xy) * u.view.zw - pivot;
    let c = cos(u.view.x);
    let s = sin(u.view.x);
    let p = pivot + vec2(c * d.x - s * d.y, s * d.x + c * d.y);
    let col = u.sky.x + p.x / (2.0 * u.sky.z);
    if (u.right.w > 0.5) {
        // All-round sky: a horizontally wrapping panorama stretched over the
        // full screen height (provisional).
        let full = vec2(col / dims.x, clamp(p.y / (480.0 * u.sky.z), 0.0, 0.999));
        let c = textureSampleLevel(tex, samp, full, 0.0);
        // Its stars glow, each twinkling at its own pace.
        let texel = floor(fract(full) * dims);
        let phase = noise(texel) * 6.2832;
        let twinkle = 0.65 + 0.35 * sin(u.sky.w * (1.5 + noise(texel.yx))  + phase);
        return vec4(c.rgb * (1.0 + glow_of(c.a) * twinkle * u.post.x), 1.0);
    }
    let row = clamp(p.y / u.sky.y, 0.0, 1.0) * dims.y;
    let uv = vec2(col / dims.x, min(row, dims.y - 0.5) / dims.y);
    return vec4(textureSampleLevel(tex, samp, uv, 0.0).rgb, 1.0);
}

// Stretch: the scene rendered small (bound as `tex`) drawn over the whole
// screen, each rendered pixel a block of screen pixels (nearest sampling).
@fragment
fn fs_stretch(v: SkyOut) -> @location(0) vec4<f32> {
    let screen = vec2<f32>(textureDimensions(tex)) * u.view.zw;
    let at = (v.clip.xy - u.origin.zw) / screen;
    return vec4(textureSampleLevel(tex, samp, at, 0.0).rgb, 1.0);
}

// Present: the glowing scene (bound as `tex`, at the view's size) onto the
// screen. Each channel clips at white on its own, so a glowing flame runs
// from red through yellow to a white core as its channels fill up (a fade
// towards white instead turned red flames pink). Then a dither of under
// half a screen step, which breaks the smooth glow gradients into noise
// instead of bands.

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3(0.0031308));
}

fn from_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

/// On an HDR screen showing up to `headroom` times white: each channel as it
/// is up to white, then above it eased into the headroom, so that the
/// brightest glow still differs from the merely bright (and, again channel
/// by channel, a flame's core turns white only at the top).
fn into_headroom(c: vec3<f32>, headroom: f32) -> vec3<f32> {
    let room = max(headroom - 1.0, 1e-3);
    let over = max(c - vec3(1.0), vec3(0.0));
    return min(c, vec3(1.0)) + room * (vec3(1.0) - exp(-over / room));
}

/// Interleaved gradient noise (Jimenez 2014): 0–1, even over the screen.
fn noise(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2(0.06711056, 0.00583715))));
}

/// `c` onto the screen at `clip`.
fn present(c: vec3<f32>, clip: vec2<f32>) -> vec4<f32> {
    if (u.post.y > 0.0) {
        return vec4(into_headroom(max(c, vec3(0.0)), u.post.y), 1.0);
    }
    // The screen stores sRGB in 8 bits: dither in those steps.
    let s = to_srgb(clamp(c, vec3(0.0), vec3(1.0))) + (noise(clip) - 0.5) * 0.8 / 255.0;
    return vec4(from_srgb(clamp(s, vec3(0.0), vec3(1.0))), 1.0);
}

@fragment
fn fs_present(v: SkyOut) -> @location(0) vec4<f32> {
    let at = (v.clip.xy - u.origin.zw) / vec2<f32>(textureDimensions(tex));
    return present(textureSampleLevel(tex, samp, at, 0.0).rgb, v.clip.xy);
}

/// With the bloom chain's top level blurred up and added: four diagonal
/// bilinear taps a texel out, half the tent of [`up`], which spares the
/// full-size picture four taps a pixel (1 ms at 2880×1440 on an Adreno 540)
/// on a level that is already blurred.
@fragment
fn fs_present_glow(v: SkyOut) -> @location(0) vec4<f32> {
    let at = (v.clip.xy - u.origin.zw) / vec2<f32>(textureDimensions(tex));
    let o = 1.0 / vec2<f32>(textureDimensions(bloom_tex));
    let taps = textureSampleLevel(bloom_tex, bloom_samp, at + o, 0.0).rgb
        + textureSampleLevel(bloom_tex, bloom_samp, at - o, 0.0).rgb
        + textureSampleLevel(bloom_tex, bloom_samp, at + vec2(o.x, -o.y), 0.0).rgb
        + textureSampleLevel(bloom_tex, bloom_samp, at + vec2(-o.x, o.y), 0.0).rgb;
    let glow = finite(taps / 4.0) * BLOOM_STRENGTH;
    return present(textureSampleLevel(tex, samp, at, 0.0).rgb + glow, v.clip.xy);
}

// Bloom: a full-screen triangle with texture coordinates over the target.

struct PostOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_post(@builtin(vertex_index) i: u32) -> PostOut {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    var o: PostOut;
    o.clip = vec4(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2(p.x, 1.0 - p.y);
    return o;
}

/// How strongly the blurred glow is added to the picture (ours, by eye).
const BLOOM_STRENGTH: f32 = 0.4;

/// `c`, or black if any channel is infinite or not a number: one such
/// texel would otherwise spread over the whole picture. Tested on the bits,
/// which fast-math compilers cannot assume away.
fn finite(c: vec3<f32>) -> vec3<f32> {
    let exponent = bitcast<vec3<u32>>(c) & vec3(0x7f800000u);
    return select(c, vec3(0.0), any(exponent == vec3(0x7f800000u)));
}

/// The part of a colour above white: only that blooms.
fn above_white(c: vec3<f32>) -> vec3<f32> {
    return max(finite(c) - vec3(1.0), vec3(0.0));
}

/// Half-size average of `tex` around `uv`: the centre and four diagonal
/// bilinear taps a texel out (dual-filter downsampling). Into a quarter-size
/// level (the chain's top on a tall screen) the same taps cover its texel's
/// 4×4 block.
fn down(uv: vec2<f32>, threshold: bool) -> vec3<f32> {
    let o = 1.0 / vec2<f32>(textureDimensions(tex));
    var taps = array<vec2<f32>, 5>(
        uv, uv + o, uv - o, uv + vec2(o.x, -o.y), uv + vec2(-o.x, o.y)
    );
    var sum = vec3(0.0);
    for (var k = 0; k < 5; k++) {
        var c = textureSampleLevel(tex, samp, taps[k], 0.0).rgb;
        if (threshold) {
            c = above_white(c);
        }
        sum += c * select(1.0, 4.0, k == 0);
    }
    return sum / 8.0;
}

/// Tent-filtered sample of the smaller level `tex` at `uv`
/// (dual-filter upsampling).
fn up(uv: vec2<f32>) -> vec3<f32> {
    let o = 1.0 / vec2<f32>(textureDimensions(tex));
    let s = textureSampleLevel(tex, samp, uv + vec2(-2.0 * o.x, 0.0), 0.0).rgb
        + textureSampleLevel(tex, samp, uv + vec2(2.0 * o.x, 0.0), 0.0).rgb
        + textureSampleLevel(tex, samp, uv + vec2(0.0, -2.0 * o.y), 0.0).rgb
        + textureSampleLevel(tex, samp, uv + vec2(0.0, 2.0 * o.y), 0.0).rgb
        + 2.0 * textureSampleLevel(tex, samp, uv + o, 0.0).rgb
        + 2.0 * textureSampleLevel(tex, samp, uv - o, 0.0).rgb
        + 2.0 * textureSampleLevel(tex, samp, uv + vec2(o.x, -o.y), 0.0).rgb
        + 2.0 * textureSampleLevel(tex, samp, uv + vec2(-o.x, o.y), 0.0).rgb;
    return s / 12.0;
}

@fragment
fn fs_bloom_first(v: PostOut) -> @location(0) vec4<f32> {
    return vec4(down(v.uv, true), 1.0);
}

@fragment
fn fs_bloom_down(v: PostOut) -> @location(0) vec4<f32> {
    return vec4(down(v.uv, false), 1.0);
}

@fragment
fn fs_bloom_up(v: PostOut) -> @location(0) vec4<f32> {
    return vec4(up(v.uv), 1.0);
}
