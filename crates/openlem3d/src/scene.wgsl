// Scene shaders: palette-textured unlit geometry, and the panned sky backdrop.

struct Uniforms {
    view_proj: mat4x4<f32>,
    // x: sky column (in sky texels) at the left edge of the screen,
    // y: horizon row in framebuffer pixels,
    // z: framebuffer pixels per original-screen pixel (height / 400),
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
    // (an HDR target, linear with 1 at white), or 0. z: 1 when the sea
    // mirrors the level (`second_tex` holds the reflection), else 0. w: 1
    // when screens are drawn as the glass of a cathode-ray tube (ours).
    post: vec4<f32>,
    // xyz: the camera's position. w: the height of the sea the level is
    // mirrored in.
    eye: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
// A second picture: the bloom chain's top level while presenting the
// glowing scene; the level mirrored in the sea while drawing the scene.
@group(1) @binding(1) var second_tex: texture_2d<f32>;
@group(1) @binding(2) var second_samp: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // x: shade (`level_mesh::SHADE_STEP`), y: 1 if palette index 0 is
    // transparent.
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
    // while glowing. On a tiled surface instead its waves (ours, enhanced
    // mode, `scene_build::Waves`): x how strongly they shade it (0 for
    // none, calm by the scenery), y how far they lift it (0 beyond the
    // finely divided sea around the level), z 0 for no water, else 1 plus
    // the surf there (1 at the shore, 0 from `FOAM_REACH` out). A negative z
    // marks a screen showing the game (`SceneLayer::screen`), uv 0–1 over it.
    @location(5) light: vec3<f32>,
};

/// Alpha steps from a plain texel to a fully glowing one (`glow.rs`).
const GLOW_STEPS: f32 = 120.0;

/// The darkest a shade takes a colour's brightest channel to, in sRGB
/// (palette index 254, the grey ramp's darkest but black).
const SHADE_FLOOR: f32 = 8.0 / 255.0;

/// A texel's glow (0–1) from its alpha (`glow.rs`).
fn glow_of(a: f32) -> f32 {
    return clamp((1.0 - a) * 255.0 / GLOW_STEPS, 0.0, 1.0);
}

/// Animation frames per second of animated sprites (unverified).
const ANIM_FPS: f32 = 8.0;

/// Frames per second of an animated sea: one per frame the original draws,
/// at its 70 Hz display (measured with marked frames on Fun 2, 2026-10-25:
/// 0, 1, 2, 3, 0, … one per captured frame).
const SEA_FPS: f32 = 70.0;

/// How far towards the camera a billboard's depth is taken, in world units.
const BILLBOARD_DEPTH_PULL: f32 = 0.1;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) params: vec2<f32>,
    // For tiled surfaces: frame count and current frame; x = 0 otherwise.
    @location(2) tile: vec2<f32>,
    @location(3) light: vec3<f32>,
    // For a tiled surface with waves: its world x and z, how strongly the
    // waves shade it, and its water and surf (`VIn::light`'s z); zero
    // otherwise.
    @location(4) wave: vec4<f32>,
    // x: 1 when drawn mirrored in the sea, y: the height above the sea.
    @location(5) mirror: vec2<f32>,
    // 1 on a screen showing the game, else 0.
    @location(6) screen: f32,
};

/// `p` mirrored in the sea when `mirrored`.
fn mirror_of(p: vec3<f32>, mirrored: bool) -> vec3<f32> {
    return select(p, vec3(p.x, 2.0 * u.eye.w - p.y, p.z), mirrored);
}

/// What the mirrored level leaves out: whatever lies this close above the
/// sea or below it (the sea itself, and the land on it).
const MIRROR_CLIP: f32 = 0.009;

@vertex
fn vs(v: VIn, @builtin(instance_index) instance: u32) -> VOut {
    var o: VOut;
    // The second instance is the level mirrored in the sea.
    let mirrored = instance == 1u;
    var frame = 0.0;
    let step = floor(u.sky.w * select(ANIM_FPS, SEA_FPS, v.anim.y < 0.0));
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
    o.wave = vec4(0.0);
    o.mirror = vec2(select(0.0, 1.0, mirrored), 0.0);
    o.screen = select(0.0, 1.0, v.light.z < 0.0);
    if (v.anim.y < 0.0) {
        var p = v.pos;
        if (v.light.y > 0.0) {
            p.y += v.light.y * WAVE_HEIGHT * waves(p.xz, t, 0.0).x;
        }
        o.clip = u.view_proj * vec4(mirror_of(p, mirrored), 1.0);
        o.light = vec3(0.0);
        o.wave = vec4(v.pos.x, v.pos.z, v.light.x, v.light.z);
        o.uv = v.uv + v.offset * u.sky.w;
        o.tile = vec2(max(v.anim.x, 1.0), frame);
        return o;
    }
    let world = v.pos + u.right.xyz * v.offset.x + vec3(0.0, v.offset.y, 0.0);
    o.mirror.y = world.y - u.eye.w;
    o.clip = u.view_proj * vec4(mirror_of(world, mirrored), 1.0);
    // A billboard is hidden only by what is in front of its middle (a
    // quarter unit above its anchor), all of it at that depth: a lemming
    // turning in a deflector's cell, half inside the block, is drawn whole,
    // as the original draws it (owner's observation). That point is taken
    // a little towards the camera, so a lemming standing in the plane of a
    // face (a turner at the edge of a step) is drawn in front of it rather
    // than fighting it for depth.
    if (any(v.offset != vec2(0.0))) {
        let towards = vec3(-u.right.z, 0.0, u.right.x) * BILLBOARD_DEPTH_PULL;
        let mid = u.view_proj
            * vec4(mirror_of(v.pos + vec3(0.0, 0.25, 0.0) + towards, mirrored), 1.0);
        o.clip.z = mid.z / mid.w * o.clip.w;
    }
    o.uv = v.uv + vec2(0.0, frame * v.anim.y);
    o.tile = vec2(0.0, 0.0);
    return o;
}

@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> {
    if (v.mirror.x > 0.5 && (v.mirror.y < MIRROR_CLIP || v.tile.x > 0.0)) {
        discard;
    }
    var uv = v.uv;
    // The sea's and the land's textures are mipmapped (`SceneLayer::smooth`).
    // A tiled surface's level of detail comes from the footprint of the uv
    // before wrapping into a frame, which would otherwise jump at each
    // repeat's edge. Magnified, the uv is snapped to texel centres, so up
    // close the texels stay sharp squares, as everywhere else (for the
    // other textures, sampled nearest, the snap changes nothing). Water
    // with waves (ours) is not snapped: it stays smooth up close.
    let size = vec2<f32>(textureDimensions(tex));
    let frames = max(v.tile.x, 1.0);
    let gx = dpdx(v.uv) / vec2(1.0, frames);
    let gy = dpdy(v.uv) / vec2(1.0, frames);
    if (v.tile.x > 0.0) {
        uv = vec2(uv.x, (fract(uv.y) + v.tile.y) / v.tile.x);
    }
    if (v.wave.w <= 0.0 && max(length(gx * size), length(gy * size)) < 1.0) {
        uv = (floor(uv * size) + 0.5) / size;
    }
    let c = textureSampleGrad(tex, samp, uv, gx, gy);
    if (v.params.y > 0.5 && c.a < 0.5) {
        discard;
    }
    // The original shades a colour down its palette ramp, whose entries
    // are the ramp's top colour scaled evenly towards black: the shade
    // (`level_mesh::SHADE_STEP`) is taken off the brightest channel of the
    // palette colour (sRGB; the sampled colour is linear) and the others
    // follow in proportion, keeping the hue. It stops short of black, at
    // the darkest grey the original shades to (palette index 254; other
    // ramps end higher, unmodelled). Glowing texels (alpha below 1) are
    // lifted above white.
    let srgb = pow(c.rgb, vec3(1.0 / 2.2));
    let top = max(srgb.r, max(srgb.g, srgb.b));
    let lowered = max(top - v.params.x, min(top, SHADE_FLOOR));
    let shaded = pow(srgb * (lowered / max(top, 1e-5)), vec3(2.2));
    let glow = glow_of(c.a);
    // Taken here, where every fragment still runs: how much of the screen
    // one pixel spans.
    let footprint = fwidth(v.uv);
    if (v.screen > 0.5 && u.post.w > 0.5) {
        return vec4(crt(v.uv, footprint), 1.0);
    }
    if (v.wave.z > 0.0 || v.wave.w > 0.0) {
        return vec4(sea(v, shaded), 1.0);
    }
    return vec4((shaded + c.rgb * v.light) * (1.0 + glow * u.post.x), 1.0);
}

// Shadows (ours, enhanced mode): soft round discs multiplied onto what is
// drawn. uv runs from −1 to 1 across a disc; params.x is how much its
// middle is darkened, fading out to nothing at its rim.
@fragment
fn fs_shadow(v: VOut) -> @location(0) vec4<f32> {
    let d = length(v.uv);
    if (d >= 1.0) {
        discard;
    }
    let dark = v.params.x * (1.0 - smoothstep(0.55, 1.0, d));
    return vec4(vec3(1.0 - dark), 1.0);
}

// Sky: a full-screen triangle. Above the horizon the sky image is stretched
// so its full height spans the screen top to the horizon; horizontally each
// sky texel covers two original-screen pixels, and the image wraps.

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    // 1 for the sky mirrored in the sea (seen below the horizon), else 0.
    @location(0) @interpolate(flat) mirrored: f32,
};

@vertex
fn vs_sky(@builtin(vertex_index) i: u32, @builtin(instance_index) instance: u32) -> SkyOut {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    var o: SkyOut;
    o.clip = vec4(p * 2.0 - 1.0, 0.0, 1.0);
    o.mirrored = select(0.0, 1.0, instance == 1u);
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
    var p = pivot + vec2(c * d.x - s * d.y, s * d.x + c * d.y);
    if (v.mirrored > 0.5) {
        p.y = 2.0 * u.sky.y - p.y;
    }
    let col = u.sky.x + p.x / (2.0 * u.sky.z);
    if (u.right.w > 0.5) {
        // All-round sky: a horizontally wrapping panorama stretched over the
        // full screen height (provisional).
        let full = vec2(col / dims.x, clamp(p.y / (400.0 * u.sky.z), 0.0, 0.999));
        let c = textureSampleLevel(tex, samp, full, 0.0);
        // Its stars glow, each twinkling at its own pace.
        let texel = floor(fract(full) * dims);
        let phase = noise(texel) * 6.2832;
        let twinkle = 0.65 + 0.35 * sin(u.sky.w * (1.5 + noise(texel.yx))  + phase);
        let lit = c.rgb * (1.0 + glow_of(c.a) * twinkle * u.post.x);
        // On an HDR screen the sky stays a backdrop: its stars clip at white
        // as on an SDR one, rather than outshining what is in front of it.
        if (u.post.y > 0.0) {
            return vec4(min(lit, vec3(1.0)), 1.0);
        }
        return vec4(lit, 1.0);
    }
    let row = clamp(p.y / u.sky.y, 0.0, 1.0) * dims.y;
    let uv = vec2(col / dims.x, min(row, dims.y - 0.5) / dims.y);
    // Mirrored, the image runs out above its top: towards there its alpha
    // fades to 0, and the sea shows that much less of it.
    let kept = select(1.0, smoothstep(0.0, SKY_MIRROR_FADE, p.y / u.sky.y), v.mirrored > 0.5);
    return vec4(textureSampleLevel(tex, samp, uv, 0.0).rgb, kept);
}

/// How much of the sky's height, from its top, the mirrored sky fades out
/// over.
const SKY_MIRROR_FADE: f32 = 0.4;

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
/// by channel, a flame's core turns white only at the top). What is above
/// white is first stretched so that a fully glowing white texel (white plus
/// the glow gain) lands at four fifths of the room: the glow is tuned for
/// SDR, where it clips, and would otherwise use little of a large headroom.
fn into_headroom(c: vec3<f32>, headroom: f32) -> vec3<f32> {
    let room = max(headroom - 1.0, 1e-3);
    // 1 - exp(-ln 5) = 4/5.
    let stretch = max(1.0, 1.609 * room / max(u.post.x, 1.0));
    let over = max(c - vec3(1.0), vec3(0.0)) * stretch;
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
    let o = 1.0 / vec2<f32>(textureDimensions(second_tex));
    let taps = textureSampleLevel(second_tex, second_samp, at + o, 0.0).rgb
        + textureSampleLevel(second_tex, second_samp, at - o, 0.0).rgb
        + textureSampleLevel(second_tex, second_samp, at + vec2(o.x, -o.y), 0.0).rgb
        + textureSampleLevel(second_tex, second_samp, at + vec2(-o.x, o.y), 0.0).rgb;
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

// Waves on the sea (ours, enhanced mode): travelling ripples summed,
// lifting the finely divided sea near the level and shading all of it by
// their slope. How strongly comes from the vertices (calm by the scenery,
// `scene_build::Waves`). So that no pattern repeats, the ripples have
// unrelated lengths and directions, each swells and fades in its own slow
// patches over the sea, and their crests are bent by a slow warp.

/// The highest a wave lifts the sea, in world units (about; patches where
/// several ripples swell together go higher).
const WAVE_HEIGHT: f32 = 0.18;
/// How much a wave's slope lightens or darkens the sea.
const WAVE_SHADE: f32 = 8.0;
/// Ripples summed.
const RIPPLE_COUNT: i32 = 8;

/// The ripples: direction in radians (x), wavelength in units (y) and
/// relative height (z).
const RIPPLES = array<vec3<f32>, 8>(
    vec3(0.0, 13.0, 0.30),
    vec3(0.65, 9.1, 0.25),
    vec3(-0.49, 7.3, 0.21),
    vec3(1.24, 5.2, 0.16),
    vec3(-0.96, 4.1, 0.13),
    vec3(0.26, 3.0, 0.10),
    vec3(1.78, 2.4, 0.08),
    vec3(-1.40, 1.9, 0.06),
);

/// Pseudo-random 0–1 for the lattice point `p` (whole numbers), mixed in
/// integers: the same for a point however it was reached, which a hash of
/// a large sine is not once the compiler reorders its arithmetic (seams
/// along the lattice).
fn hash2(p: vec2<f32>) -> f32 {
    var h = bitcast<u32>(i32(p.x)) * 73856093u ^ bitcast<u32>(i32(p.y)) * 19349663u;
    h ^= h >> 16u;
    h *= 0x7feb352du;
    h ^= h >> 15u;
    h *= 0x846ca68bu;
    h ^= h >> 16u;
    return f32(h) / 4294967295.0;
}

/// Smooth value noise, 0–1, varying over about a unit.
fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2(i), hash2(i + vec2(1.0, 0.0)), s.x),
        mix(hash2(i + vec2(0.0, 1.0)), hash2(i + vec2(1.0, 1.0)), s.x),
        s.y,
    );
}

/// The waves' height (x, about −1 to 1) and its slope along world x (y)
/// and z (z) at `p`, `t` seconds in. Ripples shorter than about three
/// times `blur` (the units a pixel covers there) fade out rather than
/// shimmer. The slope leaves out the slow patches and warp, which change
/// it little.
fn waves(p: vec2<f32>, t: f32, blur: f32) -> vec3<f32> {
    // The warp: up to about 2 units, over some 15.
    let warp = vec2(
        value_noise(p * 0.065 + vec2(3.1, 7.7) + t * 0.03),
        value_noise(p * 0.065 + vec2(11.3, 1.9) - t * 0.03),
    ) * 4.0 - 2.0;
    let q = p + warp;
    var h = vec3(0.0);
    for (var i = 0; i < RIPPLE_COUNT; i++) {
        let r = RIPPLES[i];
        let k = 6.2832 / r.y;
        let d = vec2(cos(r.x), sin(r.x));
        // Deep-water waves: their speed grows with the root of the length.
        let w = sqrt(9.8 * k) * 0.55;
        // Each ripple's own patches, some 20 units across, drifting slowly.
        let swell = value_noise(p * 0.05 + vec2(f32(i) * 17.3, f32(i) * 5.1) + t * 0.02);
        let a = r.z * (0.25 + 1.5 * swell * swell)
            * (1.0 - smoothstep(r.y / 10.0, r.y / 3.0, blur));
        let phase = k * dot(d, q) - w * t + f32(i) * 1.7;
        h += a * vec3(sin(phase), k * d.x * cos(phase), k * d.y * cos(phase));
    }
    return h;
}

/// How much of the mirrored level the sea shows looking straight down, and
/// looking along it towards the horizon (ours, tuned by eye).
const REFLECT_DOWN: f32 = 0.08;
const REFLECT_GRAZING: f32 = 0.45;
/// How far the waves' slope shifts the reflection, in screen heights per
/// unit of slope.
const REFLECT_RIPPLE: f32 = 0.025;
/// The surf's colour, linear.
const FOAM_COLOUR: vec3<f32> = vec3(0.85, 0.9, 0.92);

/// Surf at `p`, `t` seconds in, `shore` (0–1) being how near the scenery
/// it is: how much froth covers it (x), the froth's height (y, 0–1, for
/// its shading) and how much froth there would be a little further out
/// (z, for the darkening around it).
fn surf(p: vec2<f32>, t: f32, shore: f32) -> vec3<f32> {
    let froth = 0.6 * value_noise(p * 3.1 + vec2(t * 0.21, -t * 0.17))
        + 0.4 * value_noise(p * 7.3 - vec2(t * 0.13, t * 0.29));
    let reach = shore + (froth - 0.5) * 0.35;
    let wash = 0.5 + 0.22 * sin(t * 0.8 + value_noise(p * 0.25) * 6.0);
    return vec3(
        froth_cover(reach, wash, froth, 0.0),
        froth,
        froth_cover(reach, wash, froth, 0.12),
    );
}

/// How much froth covers the sea `reach` (0–1) towards the shore, with the
/// washing line at `wash` and the froth there `froth` (0–1); `widen`
/// grows it outwards.
fn froth_cover(reach: f32, wash: f32, froth: f32, widen: f32) -> f32 {
    let r = reach + widen;
    let line = smoothstep(wash - 0.1, wash, r) * (1.0 - smoothstep(wash, wash + 0.06, reach));
    let rim = smoothstep(0.78, 0.98, r);
    let gate = smoothstep(0.3 - widen, 0.6 - widen, froth);
    return max(rim * 0.95 * mix(0.6, 1.0, gate), line * 0.7 * gate);
}

/// The sea's colour where `v` lies on it, from its texture's `shaded`
/// colour: lit by the waves' slope from a fixed light, crests a touch
/// lighter; on water, mirroring the level more towards the horizon,
/// rippled by the waves, and surf by the shore washing in and out.
fn sea(v: VOut, shaded: vec3<f32>) -> vec3<f32> {
    let p = v.wave.xy;
    let t = u.sky.w;
    let blur = length(fwidth(p));
    let w = waves(p, t, blur) * v.wave.z;
    let slope = w.yz * WAVE_HEIGHT * WAVE_SHADE;
    let n = normalize(vec3(-slope.x, 1.0, -slope.y));
    let l = normalize(vec3(-0.5, 1.0, -0.35));
    var c = shaded * (dot(n, l) / dot(vec3(0.0, 1.0, 0.0), l) + 0.06 * w.x);
    let water = v.wave.w > 0.0;
    if (water && u.post.z > 0.0) {
        let to_eye = normalize(u.eye.xyz - vec3(p.x, u.eye.w, p.y));
        let facing = clamp(dot(n, to_eye), 0.0, 1.0);
        let amount = mix(REFLECT_DOWN, REFLECT_GRAZING, pow(1.0 - facing, 3.0));
        let dims = vec2<f32>(textureDimensions(second_tex));
        let at = (v.clip.xy + n.xz * REFLECT_RIPPLE * dims.y) / dims;
        let mirrored = textureSampleLevel(second_tex, second_samp, at, 0.0);
        c = mix(c, mirrored.rgb, amount * clamp(mirrored.a, 0.0, 1.0));
    }
    if (water) {
        // Surf: froth at the shore, and a line of it washing in and out,
        // both frayed and torn by drifting noise; faded out where a pixel
        // spans much of a unit. The froth is lit as a bumpy surface from
        // the waves' light, thin froth lets the sea through, and the water
        // just around it is darkened, as if it stood a little proud.
        let shore = clamp(v.wave.w - 1.0, 0.0, 1.0);
        let fade = 1.0 - smoothstep(0.3, 0.8, blur);
        let here = surf(p, t, shore);
        // The same a little towards the light, for the froth's slope.
        let towards = surf(p + l.xz * 0.06, t, shore);
        let bump = clamp((towards.y - here.y) * 6.0, -1.0, 1.0);
        let foam = here.x * fade;
        let around = clamp(here.z - here.x, 0.0, 1.0) * fade;
        c *= 1.0 - 0.35 * around;
        let lit = FOAM_COLOUR * (0.82 + 0.3 * bump) * (0.85 + 0.15 * here.y);
        c = mix(c, lit, foam);
    }
    return c;
}

// A screen showing the game as the glass of a cathode-ray tube (ours,
// enhanced mode): bulging glass with rounded corners, scanlines and a
// phosphor mask (both fading out where they would be finer than the
// pixels), colours fringing apart towards the edges, darker corners, a
// faint band of brightness rolling down and a sheen on the glass.

/// How far the glass bulges: the picture's corners pull in by about this
/// much of its size.
const CRT_CURVE: f32 = 0.12;
/// The rounding of the glass's corners, as a share of the picture.
const CRT_CORNER: f32 = 0.06;
/// Scanlines down the picture (the original's 200 rows) and phosphor
/// triads across it.
const CRT_LINES: f32 = 200.0;
const CRT_TRIADS: f32 = 320.0;
/// How far the red and blue images drift apart at the edges, as a share
/// of the picture.
const CRT_FRINGE: f32 = 0.006;
/// How much brighter than the picture the tube is drawn, against the
/// light the scanlines and mask take away.
const CRT_GAIN: f32 = 1.3;

/// The tube's colour at `at` (0–1 over the picture), one pixel spanning
/// `footprint` of it.
fn crt(at: vec2<f32>, footprint: vec2<f32>) -> vec3<f32> {
    let t = u.sky.w;
    let d = at - 0.5;
    let uv = 0.5 + d * (1.0 + CRT_CURVE * 4.0 * dot(d, d));
    // The glass: a rectangle with rounded corners, softened over a pixel.
    let q = abs(uv - 0.5) - (0.5 - CRT_CORNER);
    let edge = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - CRT_CORNER;
    let pixel = max(footprint.x, footprint.y);
    let glass = 1.0 - smoothstep(-pixel, pixel, edge);
    let fringe = d * CRT_FRINGE * 2.0;
    var c = vec3(
        textureSampleLevel(tex, samp, uv + fringe, 0.0).r,
        textureSampleLevel(tex, samp, uv, 0.0).g,
        textureSampleLevel(tex, samp, uv - fringe, 0.0).b,
    );
    // Scanlines and mask, faded out as they near a pixel's size.
    let lines = 1.0 - smoothstep(0.25, 0.5, footprint.y * CRT_LINES);
    let scan = 0.5 + 0.5 * cos(uv.y * CRT_LINES * 6.2832);
    c *= mix(1.0, 0.45 + 0.55 * scan, lines);
    let triads = 1.0 - smoothstep(0.2, 0.4, footprint.x * CRT_TRIADS);
    let k = uv.x * CRT_TRIADS * 6.2832;
    let mask = 0.7 + 0.3 * vec3(cos(k), cos(k - 2.0944), cos(k - 4.1888));
    c *= mix(vec3(1.0), mask, triads);
    c *= CRT_GAIN;
    // Darker towards the corners, a faint band rolling down, a sheen.
    c *= 1.0 - 0.5 * pow(length(d) * 1.4, 4.0);
    let band = fract(uv.y * 0.6 - t * 0.12);
    c *= 1.0 + 0.05 * smoothstep(0.0, 0.08, band) * (1.0 - smoothstep(0.08, 0.3, band));
    c += vec3(0.05) * (1.0 - smoothstep(0.0, 0.45, length(at - vec2(0.28, 0.22))));
    return c * glass;
}
