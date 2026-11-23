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
    // x: 1 when a sandstorm blows over the level (ours), else 0. y: how
    // much of it blows past the camera, 0–1: less under cover. z: 1 when
    // distant things shimmer in the heat (ours), else 0. w: 1 when the
    // all-round sky is a night sky of stars shown with our touches, else 0.
    weather: vec4<f32>,
    // x: 1 when the floor shines with the level mirrored at `eye.w` (ours),
    // 2 when only the tops of slippery blocks there do, else 0; w how much
    // (1 for a polished floor). y: the circuit board's signal field's
    // texels per unit, z: 1 when that field is bound as `field_tex`
    // (`crate::signals`), else 0.
    gloss: vec4<f32>,
    // x: 1 when snow falls (ours), else 0. y: how much falls around the
    // camera, 0–1: none under cover. z: the marks field's texels per unit
    // (`crate::marks`), 0 without. w: 1 when windows look out on space
    // (ours, `space`), else 0.
    snow: vec4<f32>,
    // From clip space back to the world.
    inv_view_proj: mat4x4<f32>,
    // On a level with a sandstorm, the grid cells holding a visible block:
    // word y · 32 + z (four to an element), bit x (`SceneData::solid`).
    solid: array<vec4<u32>, 128>,
};

/// Whether the grid cell at `p` (world) holds a visible block
/// (`Uniforms::solid`); outside the grid, no.
fn cell_solid(p: vec3<f32>) -> bool {
    return cell_solid_at(vec3<i32>(floor(p)));
}

/// Whether grid cell `c` holds a visible block (`Uniforms::solid`); outside
/// the grid, no.
fn cell_solid_at(c: vec3<i32>) -> bool {
    if (any(c < vec3(0)) || c.x >= 32 || c.y >= 16 || c.z >= 32) {
        return false;
    }
    let w = c.y * 32 + c.z;
    return (u.solid[w / 4][w % 4] >> u32(c.x) & 1u) != 0u;
}

/// Whether, looking in direction `d` through a window's glass at `p`, more
/// of the level shows behind it: past the run of blocks the window is in,
/// the line of sight meets another visible block before leaving the grid.
fn sees_level(p: vec3<f32>, d: vec3<f32>) -> bool {
    let start = p + d * 1e-3;
    var cell = vec3<i32>(floor(start));
    let step = vec3<i32>(sign(d));
    let inv = 1.0 / max(abs(d), vec3(1e-6));
    // How far along the line the next cell boundary is on each axis.
    let next = vec3<f32>(cell) + select(vec3(0.0), vec3(1.0), d > vec3(0.0));
    var t_max = abs(next - start) * inv;
    let t_delta = inv;
    // Still in the blocks the window is set in.
    var in_wall = true;
    for (var i = 0; i < 96; i++) {
        if (any(cell < vec3(-1)) || cell.x > 32 || cell.y > 16 || cell.z > 32) {
            return false;
        }
        let solid = cell_solid_at(cell);
        if (in_wall) {
            in_wall = solid;
        } else if (solid) {
            return true;
        }
        if (t_max.x < t_max.y && t_max.x < t_max.z) {
            cell.x += step.x;
            t_max.x += t_delta.x;
        } else if (t_max.y < t_max.z) {
            cell.y += step.y;
            t_max.y += t_delta.y;
        } else {
            cell.z += step.z;
            t_max.z += t_delta.z;
        }
    }
    return false;
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
// A second picture: the bloom chain's top level while presenting the
// glowing scene; the level mirrored in the sea while drawing the scene.
@group(1) @binding(1) var second_tex: texture_2d<f32>;
@group(1) @binding(2) var second_samp: sampler;
// While drawing the scene, the circuit board's signal field
// (`crate::signals`), or a blank texel.
@group(1) @binding(3) var field_tex: texture_2d<f32>;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // x: shade (`level_mesh::SHADE_STEP`), y: 1 if palette index 0 is
    // transparent, −1 on the top of a slippery block, −2 less a thousandth
    // of the tile on a face showing a whole window tile (looking out on
    // `space`), else 0.
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
    // marks a screen showing the game (`SceneLayer::screen`), uv 0–1 over it,
    // -2 a circuit board, which the lemmings' signals light
    // (`crate::signals`), and -3 snow lying on the level, which their marks
    // press down (`scene_build::SNOW_DEPTH`): x then its depth there, y the
    // height of the ground under it.
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
    // Where it is in the world, before mirroring.
    @location(7) world: vec3<f32>,
    // 1 on a circuit board, else 0.
    @location(8) circuit: f32,
    // On snow lying on the level (ours): x 1, y how far it is pressed down
    // there, 0–1 of its depth, z how far it is moved up from where it lies
    // untouched; else 0.
    @location(9) cover: vec3<f32>,
};

/// `p` mirrored in the sea when `mirrored`.
fn mirror_of(p: vec3<f32>, mirrored: bool) -> vec3<f32> {
    return select(p, vec3(p.x, 2.0 * u.eye.w - p.y, p.z), mirrored);
}

/// What the mirrored level leaves out: whatever lies this close above a sea
/// or below it (the sea itself, and the land on it); under a glossy floor,
/// which lies just below its mirror, only what is below the mirror.
const MIRROR_CLIP: f32 = 0.009;
const GLOSS_CLIP: f32 = -0.001;

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
    o.screen = select(0.0, 1.0, v.light.z < 0.0 && v.light.z > -1.5);
    o.circuit = select(0.0, 1.0, v.light.z < -1.5 && v.light.z > -2.5);
    o.cover = vec3(0.0);
    if (v.anim.y < 0.0) {
        var p = v.pos;
        if (v.light.y > 0.0) {
            p.y += v.light.y * WAVE_HEIGHT * waves(p.xz, t, 0.0).x;
        }
        o.clip = u.view_proj * vec4(mirror_of(p, mirrored), 1.0);
        o.light = vec3(0.0);
        o.wave = vec4(v.pos.x, v.pos.z, v.light.x, v.light.z);
        o.world = p;
        o.uv = v.uv + v.offset * u.sky.w;
        o.tile = vec2(max(v.anim.x, 1.0), frame);
        return o;
    }
    var world = v.pos + u.right.xyz * v.offset.x + vec3(0.0, v.offset.y, 0.0);
    if (v.light.z < -2.5) {
        // Snow lying on the level, pressed down by the lemmings' marks.
        let pressed_here = snow_pressed(world, v.light.x, v.light.y);
        world.y += pressed_here.x;
        o.cover = vec3(1.0, pressed_here.y, pressed_here.x);
        o.light = vec3(0.0);
    }
    o.mirror.y = world.y - u.eye.w;
    o.world = world;
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

// What a level's scene shows (ours): its geometry's pipelines are compiled
// with only these on (`scene_render::Features`), so that no pixel pays for
// the rest; a weak GPU spends much of its frame on them otherwise.
override WINDOWS: bool = true;
override SCREENS: bool = true;
override SEA: bool = true;
override CIRCUIT: bool = true;
override SNOW: bool = true;
override GLOSS: bool = true;
override STORM: bool = true;
override MIRROR: bool = true;

@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> {
    // A glossy floor lies just under its mirror, which leaves out only what
    // is below it, so that blocks meet their reflections.
    let clip = select(MIRROR_CLIP, GLOSS_CLIP, u.gloss.x > 0.5);
    if (MIRROR && v.mirror.x > 0.5 && (v.mirror.y < clip || v.tile.x > 0.0)) {
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
    // On a window's face (`VIn::params`, its tile in thousandths) the
    // texture is taken from inside the tile only: at the face's edge a
    // pixel can fall a little beyond it, onto the frame across the tile.
    if (WINDOWS && v.params.y < -1.5) {
        let tile = round(fract(-v.params.y) * 1000.0);
        let half = 0.5 / size;
        uv = clamp(uv, vec2(half.x, tile / TILES + half.y), vec2(1.0 - half.x, (tile + 1.0) / TILES - half.y));
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
    // A window's glass (its alpha marked, `scene_build::WINDOW_ALPHA`)
    // looks out on space.
    let window = WINDOWS && u.snow.w > 0.5 && v.params.y < -1.5 && c.a > 0.15 && c.a < 0.4;
    // Glass not shown as a window (on a face showing a sliver of one)
    // stays as painted, without glow.
    let glass = v.params.y < 0.5 && c.a > 0.15 && c.a < 0.4;
    let glow = select(glow_of(c.a), 0.0, glass);
    // Taken here, where every fragment still runs: how much of the screen
    // one pixel spans.
    let footprint = fwidth(v.uv);
    // Which way the eye looks through this pixel, and how much of that
    // direction one pixel spans (`pixel_angle`: the derivatives, taken only
    // where a window is drawn, are unreliable along its edge).
    let look = normalize(v.world - u.eye.xyz);
    let look_px = pixel_angle();
    // Which way the surface faces, from how its position changes across
    // the screen.
    let facing = normalize(cross(dpdx(v.world), dpdy(v.world)));
    // Which way snow lying on the level faced untouched.
    let untouched = v.world - vec3(0.0, v.cover.z, 0.0);
    let ground = normalize(cross(dpdx(untouched), dpdy(untouched)) + vec3(0.0, 1e-9, 0.0));
    // Glass with more of the level behind it stays as painted.
    if (WINDOWS && window && v.mirror.x < 0.5 && !sees_level(v.world, look)) {
        // The glass as painted (near black, with light glare streaks)
        // shows as a reflection over the view.
        return vec4(space(look, look_px) + shaded, 1.0);
    }
    if (SCREENS && v.screen > 0.5 && u.post.w > 0.5) {
        return vec4(crt(v.uv, footprint), 1.0);
    }
    if (SEA && (v.wave.z > 0.0 || v.wave.w > 0.0)) {
        return vec4(sea(v, shaded), 1.0);
    }
    var lit = (shaded + c.rgb * v.light) * (1.0 + glow * u.post.x);
    if (CIRCUIT && v.circuit > 0.5) {
        let texel = max(length(gx * size), length(gy * size));
        lit += sent_signal(v.world, texel);
    }
    if (SNOW && v.cover.x > 0.5) {
        lit = snow_lit(lit, facing, ground, v.cover.y);
    } else if (SNOW && u.snow.z > 0.0 && v.mirror.x < 0.5 && abs(facing.y) > 0.7) {
        lit = marked(lit, v.world, srgb);
    }
    if (GLOSS && u.gloss.x > 0.5 && v.mirror.x < 0.5) {
        lit = glossy(lit, v, facing);
    }
    if (STORM && u.weather.x > 0.0) {
        // Sand drifting over the desert floor, then the dust in the air.
        if (v.tile.x > 0.0) {
            lit *= drift(v.world.xz);
        }
        lit = dust(lit, v.world);
    }
    return vec4(lit, select(1.0, heat_of(v.world), STORM));
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
        let starry = u.weather.w > 0.5;
        var c = textureSampleLevel(tex, samp, full, 0.0);
        // In a night sky only a point of light is a star (ours): a glow with
        // more glow a couple of texels off is part of something bigger (a
        // planet), which neither twinkles, texel by texel in bands, nor
        // outshines its own dim rim.
        var around = 0.0;
        if (starry) {
            let o = STAR_SPAN / dims;
            around = max(
                max(
                    glow_of(textureSampleLevel(tex, samp, full + vec2(o.x, 0.0), 0.0).a),
                    glow_of(textureSampleLevel(tex, samp, full - vec2(o.x, 0.0), 0.0).a),
                ),
                max(
                    glow_of(textureSampleLevel(tex, samp, full + vec2(0.0, o.y), 0.0).a),
                    glow_of(textureSampleLevel(tex, samp, full - vec2(0.0, o.y), 0.0).a),
                ),
            );
        }
        // Its stars glow, each twinkling at its own pace.
        let texel = floor(fract(full) * dims);
        let phase = noise(texel) * 6.2832;
        var twinkle = 0.65 + 0.35 * sin(u.sky.w * (1.5 + noise(texel.yx))  + phase);
        var point = 0.0;
        if (starry) {
            point = 1.0 - smoothstep(0.15, 0.5, around);
            twinkle = mix(0.82, twinkle, point);
        }
        var lit = c.rgb * (1.0 + glow_of(c.a) * twinkle * u.post.x);
        if (starry) {
            // Twinkling in brightness too, without glow as well (ours).
            lit *= 1.0 + glow_of(c.a) * (twinkle - 0.8) * 1.5;
            // Big glowing bodies breathe slowly, each as a whole (ours).
            lit *= 1.0 + glow_of(c.a) * (1.0 - point) * breath(full);
        }
        // On an HDR screen the sky stays a backdrop: it clips at white as on
        // an SDR one, rather than outshining what is in front of it. Only a
        // night sky's stars shine on above white (ours), as stars do.
        if (u.post.y > 0.0) {
            var shown = min(lit, vec3(1.0));
            if (starry) {
                shown += c.rgb * glow_of(c.a) * twinkle * point * STAR_HDR_GAIN;
            }
            return vec4(shown, 1.0);
        }
        return vec4(lit, 1.0);
    }
    let row = clamp(p.y / u.sky.y, 0.0, 1.0) * dims.y;
    let uv = vec2(col / dims.x, min(row, dims.y - 0.5) / dims.y);
    // Mirrored, the image runs out above its top: towards there its alpha
    // fades to 0, and the sea shows that much less of it.
    let kept = select(1.0, smoothstep(0.0, SKY_MIRROR_FADE, p.y / u.sky.y), v.mirrored > 0.5);
    var sky = textureSampleLevel(tex, samp, uv, 0.0).rgb;
    if (u.weather.x > 0.0) {
        sky = dusty_sky(sky, p, col);
    }
    // With heat shimmer the alpha is how much the sky shimmers instead.
    return vec4(sky, select(kept, heat_of_sky(p), u.weather.z > 0.5));
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
    return present(textureSampleLevel(tex, samp, shimmer(at), 0.0).rgb, v.clip.xy);
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
    return present(textureSampleLevel(tex, samp, shimmer(at), 0.0).rgb + glow, v.clip.xy);
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

// A sandstorm over a desert (ours, Ultra mode): dust in the air, thicker
// with distance and near the ground and billowing as the wind carries it;
// streaks of sand racing over the floor; the sky dusty down to the
// horizon, with bands blowing past; sheets of dust sweeping across the
// view; and grains of sand flying along the wind (`vs_sand`).
// The whole storm swells and eases in gusts.

/// The wind's direction over the ground (world x and z).
const WIND: vec2<f32> = vec2(0.944, 0.330);
/// The dust's colour, linear, and the sky's where the dust hides it.
const DUST_COLOUR: vec3<f32> = vec3(0.60, 0.43, 0.25);
const DUST_SKY: vec3<f32> = vec3(0.78, 0.58, 0.34);
/// How thick the dust is: the share of light it takes per unit, at an
/// average gust.
const DUST_DENSITY: f32 = 0.06;
const GRAIN_COLOUR: vec3<f32> = vec3(0.85, 0.68, 0.45);
/// How far from the eye the air is clear of dust, in units, so that what
/// is close stays clear.
const DUST_CLEAR: f32 = 3.0;

/// How strong the gusts are `t` seconds in, about 0.3 to 1.
fn gust(t: f32) -> f32 {
    return clamp(0.65 + 0.25 * sin(t * 0.31) + 0.15 * sin(t * 0.83 + 1.7), 0.3, 1.0);
}

/// How strong the storm is `t` seconds in: the gusts.
fn storm(t: f32) -> f32 {
    return gust(t) * u.weather.x;
}

/// How much brighter the desert floor is at `p` (world x, z) under the sand
/// blowing over it.
fn drift(p: vec2<f32>) -> f32 {
    let t = u.sky.w;
    let along = dot(p, WIND);
    let across = dot(p, vec2(-WIND.y, WIND.x));
    let n = value_noise(vec2(along * 0.35 - t * 2.2, across * 2.5))
        * value_noise(vec2(along * 0.9 - t * 3.1, across * 1.3 + 5.0));
    return 1.0 + 0.6 * smoothstep(0.2, 0.55, n) * storm(t);
}

/// `c`, seen through the dust between the eye and `world`.
fn dust(c: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let t = u.sky.w;
    let d = max(distance(world, u.eye.xyz) - DUST_CLEAR, 0.0);
    let low = exp(-max(world.y - 1.0, 0.0) / 5.0);
    let billow = value_noise((world.xz - WIND * t * 3.0) * 0.08);
    let density = DUST_DENSITY * storm(t) * (0.5 + billow) * (0.5 + 0.5 * low);
    let fog = 1.0 - exp(-d * density);
    return mix(c, DUST_COLOUR, fog * 0.92);
}

/// The sky `sky` behind the dust, at `p` (screen pixels, the horizon at
/// `u.sky.y`) and sky column `col`.
fn dusty_sky(sky: vec3<f32>, p: vec2<f32>, col: f32) -> vec3<f32> {
    let t = u.sky.w;
    let r = p.y / max(u.sky.y, 1.0);
    let near = smoothstep(0.1, 1.0, r);
    let bands = value_noise(vec2(col * 0.02 - t * 0.6, r * 7.0));
    let amount = storm(t) * (0.55 + 0.45 * near) * (0.85 + 0.3 * bands);
    // Desaturated first, so blue sky and brown dust do not mix to purple.
    let grey = vec3(dot(sky, vec3(0.3, 0.55, 0.15)));
    return mix(mix(sky, grey, min(1.2 * storm(t), 1.0)), DUST_SKY, clamp(amount, 0.0, 0.97));
}

/// Sheets of dust sweeping across the view, long along the wind as it
/// crosses the screen: faster and more sideways the more across the view
/// the wind blows, turning smoothly as the camera turns.
@fragment
fn fs_dust_sheets(v: SkyOut) -> @location(0) vec4<f32> {
    let t = u.sky.w;
    let g = storm(t) * u.weather.y;
    let side = dot(WIND, normalize(u.right.xz + vec2(1e-5, 0.0)));
    let dir = normalize(vec2(side, 0.12));
    let p = v.clip.xy / u.sky.z;
    let along = dot(p, dir);
    let across = dot(p, vec2(-dir.y, dir.x));
    let speed = 0.3 + 0.7 * abs(side);
    let sheet = value_noise(vec2(along * 0.006 - t * 1.4 * speed, across * 0.02))
        * value_noise(vec2(along * 0.013 - t * 2.3 * speed, across * 0.045 + 9.0));
    let sheets = smoothstep(0.15, 0.5, sheet) * 0.2 * g;
    return vec4(DUST_SKY, sheets);
}

// Sand grains in a sandstorm (ours, Ultra): each has its own place in the
// world, low over the ground, and is carried along the wind, in a box that
// wraps around the camera as the snow's does (moving, the camera passes
// among them). Drawn as short streaks along their flight, hidden by what
// is nearer.

/// How many grains there are; the box's width, in units; how high over
/// the ground they fly; how fast, in units a second; how long a streak is,
/// in seconds of flight; how thick, in units.
const SAND_GRAINS: u32 = 4000u;
const SAND_BOX: f32 = 14.0;
const SAND_HIGH: f32 = 2.5;
const SAND_SPEED: f32 = 6.0;
const SAND_STREAK: f32 = 0.04;
const SAND_WIDTH: f32 = 0.012;
/// How far upwind a block keeps the grains away, in units (looked along
/// in half-unit steps); how far under a roof; how high above a grain a
/// block is its roof, in units.
const SAND_LEE: f32 = 3.0;
const SAND_LEE_INDOORS: f32 = 16.0;
const SAND_ROOF: i32 = 6;

struct SandOut {
    @builtin(position) clip: vec4<f32>,
    // Where on the streak: along it and across it, -1 to 1; its opacity.
    @location(0) at: vec2<f32>,
    @location(1) alpha: f32,
};

@vertex
fn vs_sand(@builtin(vertex_index) i: u32, @builtin(instance_index) n: u32) -> SandOut {
    let t = u.sky.w;
    let k = f32(n);
    let seed = vec3(hash2(vec2(k, 11.0)), hash2(vec2(k, 12.0)), hash2(vec2(k, 13.0)));
    let speed = SAND_SPEED * (0.6 + 0.8 * hash2(vec2(k, 14.0)));
    let phase = hash2(vec2(k, 15.0)) * 6.283;
    // Along the wind, bouncing a little, from its own place in the world.
    var p = vec3(seed.x * SAND_BOX, 1.0 + 0.05 + seed.y * seed.y * SAND_HIGH, seed.z * SAND_BOX);
    p.x += WIND.x * speed * t;
    p.z += WIND.y * speed * t;
    p.y += 0.08 * sin(t * 3.0 + phase);
    // Wrapped across the ground into the box around the camera.
    let half = SAND_BOX * 0.5;
    let rel_xz = p.xz - u.eye.xz;
    let wrapped = rel_xz + half - floor((rel_xz + half) / SAND_BOX) * SAND_BOX - half;
    let world = vec3(u.eye.x + wrapped.x, p.y, u.eye.z + wrapped.y);
    let rel = world - u.eye.xyz;
    let reach = max(abs(wrapped.x), abs(wrapped.y)) / half;
    var alpha = (1.0 - smoothstep(0.6, 1.0, reach)) * smoothstep(0.2, 0.6, length(rel));
    // Only some grains fly at a time: more as the storm gusts.
    let g = storm(t);
    alpha *= select(0.0, 1.0, hash2(vec2(k, 16.0)) < 0.25 + 0.75 * g) * g;
    // None in a block, nor in the lee of one: where a block stands a
    // little way upwind, the wind brings no sand. Under a roof the wind
    // must come a long way unbroken (into a tomb, it does not; under a
    // roof open to the wind, it does).
    var roofed = false;
    for (var s = 1; s <= SAND_ROOF; s++) {
        roofed = roofed || cell_solid(world + vec3(0.0, f32(s), 0.0));
    }
    let lee = select(SAND_LEE, SAND_LEE_INDOORS, roofed);
    let steps = i32(lee * 2.0);
    for (var s = 0; s <= steps && alpha > 0.0; s++) {
        let back = f32(s) * 0.5;
        if (cell_solid(world - vec3(WIND.x, 0.0, WIND.y) * back)) {
            alpha = 0.0;
        }
    }
    // A streak along the flight, at least about a pixel thick (fainter
    // for it).
    let along = normalize(vec3(WIND.x, 0.24 * cos(t * 3.0 + phase), WIND.y));
    let to_eye = normalize(-rel);
    let across = normalize(cross(along, to_eye) + vec3(0.0, 1e-5, 0.0));
    let c = u.view_proj * vec4(world, 1.0);
    let c_up = u.view_proj * vec4(world + vec3(0.0, 1.0, 0.0), 1.0);
    let per_unit = abs(c_up.y / c_up.w - c.y / c.w) * 200.0 * u.sky.z;
    let width = max(SAND_WIDTH, 0.8 / max(per_unit, 1e-3));
    alpha *= min(1.0, SAND_WIDTH / width) * 0.7;
    let corner = vec2(f32((0x32u >> i) & 1u), f32((0x2Cu >> i) & 1u)) * 2.0 - 1.0;
    let length_half = speed * SAND_STREAK * 0.5;
    let at = world + along * corner.x * length_half + across * corner.y * width;
    var o: SandOut;
    o.clip = u.view_proj * vec4(at, 1.0);
    o.at = corner;
    o.alpha = alpha;
    return o;
}

@fragment
fn fs_sand(v: SandOut) -> @location(0) vec4<f32> {
    // Brightest at the head, trailing off behind; soft across.
    let a = (1.0 - abs(v.at.y)) * (0.35 + 0.65 * (v.at.x * 0.5 + 0.5)) * v.alpha;
    if (a <= 0.0) {
        discard;
    }
    return vec4(GRAIN_COLOUR, a);
}

// Heat shimmer over a desert (ours, Ultra mode): what is far off and near
// the ground wavers, in waves rising slowly up the screen. While drawing,
// the scene keeps in each pixel's alpha how much it shimmers; the picture
// is then shifted by that much as it is presented.

/// Where the shimmer starts and where it is full, in units from the eye.
const HEAT_NEAR: f32 = 8.0;
const HEAT_FAR: f32 = 30.0;
/// How far the picture shifts at full shimmer, in the original's pixels.
const HEAT_PIXELS: f32 = 1.2;

/// The alpha kept for what is drawn at `world`: how much it shimmers, or 1
/// without heat.
fn heat_of(world: vec3<f32>) -> f32 {
    if (u.weather.z <= 0.5) {
        return 1.0;
    }
    let far = smoothstep(HEAT_NEAR, HEAT_FAR, distance(world, u.eye.xyz));
    let low = exp(-max(world.y - 1.0, 0.0) / 4.0);
    return far * (0.35 + 0.65 * low);
}

/// How much the sky shimmers at `p` (screen pixels, the horizon at
/// `u.sky.y`): only just above the horizon.
fn heat_of_sky(p: vec2<f32>) -> f32 {
    return smoothstep(0.8, 1.0, p.y / max(u.sky.y, 1.0));
}

/// Where to take the picture from for the point `at` (0–1 over it), shifted
/// by the shimmer kept in its alpha.
fn shimmer(at: vec2<f32>) -> vec2<f32> {
    if (u.weather.z <= 0.5) {
        return at;
    }
    let dims = vec2<f32>(textureDimensions(tex));
    let amount = clamp(textureSampleLevel(tex, samp, at, 0.0).a, 0.0, 1.0);
    let p = at * dims / u.sky.z;
    let t = u.sky.w;
    let wobble = 0.6 * sin(p.y * 0.9 + t * 7.0 + 2.0 * sin(p.x * 0.05 + t))
        + 0.4 * sin(p.y * 2.3 + t * 11.0 + p.x * 0.11);
    let offset = vec2(wobble, 0.25 * sin(p.x * 0.7 + t * 5.0)) * HEAT_PIXELS * u.sky.z * amount;
    // Kept inside the picture: the sampler would wrap round its edges.
    let shifted = clamp(at + offset / dims, vec2(0.5) / dims, vec2(1.0) - vec2(0.5) / dims);
    // Only shifted onto what shimmers as much: else, beside something near
    // (a wall's edge), that would be pulled into what is far behind it.
    let there = clamp(textureSampleLevel(tex, samp, shifted, 0.0).a, 0.0, 1.0);
    return mix(at, shifted, smoothstep(amount - 0.3, amount - 0.05, there));
}

// A night sky (ours, Enhanced and Ultra): its stars twinkle in brightness,
// shining above white on an HDR screen.

/// How far above white a night sky's stars shine on an HDR screen, per
/// unit of their brightness (ours, by eye).
const STAR_HDR_GAIN: f32 = 4.0;
/// How far a big glowing body's brightness swells and dims as it breathes.
const BREATH: f32 = 0.18;
/// How fast it breathes, in radians a second (a breath every 12 s or so).
const BREATH_PACE: f32 = 0.5;

/// How much brighter (or dimmer) a big glowing body at `uv` (0–1 over the
/// panorama, wrapping across) is just now, about ±[`BREATH`]: its timing
/// varies slowly across the sky, so that a body breathes as one and
/// bodies apart are out of step.
fn breath(uv: vec2<f32>) -> f32 {
    let a = 6.2832 * uv.x;
    let phase = 1.5 * sin(a + uv.y * 2.0) + sin(2.0 * a - uv.y * 3.0 + 1.3);
    return BREATH * sin(u.sky.w * BREATH_PACE + phase);
}

/// How far off, in sky texels, a star is looked at for more glow: further
/// than a star spans, nearer than a planet's edge.
const STAR_SPAN: f32 = 2.5;

// A glossy floor (ours, Ultra mode: "Breakout", and seas of ice, fainter):
// it shows the level mirrored in it, as a polished floor does, more towards
// the horizon.

/// How much of the mirrored level a glossy floor shows looking straight
/// down, and looking along it.
const GLOSS_DOWN: f32 = 0.12;
const GLOSS_GRAZING: f32 = 0.4;

/// `lit`, the colour of the fragment `v` facing `facing`, with the mirrored
/// level in it if it is the floor.
fn glossy(lit: vec3<f32>, v: VOut, facing: vec3<f32>) -> vec3<f32> {
    // Only the floor, at the mirror: block tops above it would show the
    // picture misplaced.
    if (abs(facing.y) < 0.98 || abs(v.world.y - u.eye.w) > 0.02 || u.eye.y < v.world.y) {
        return lit;
    }
    // Ice among other floors: only the slippery tops (`VIn::params`).
    if (u.gloss.x > 1.5 && (v.params.y > -0.5 || v.params.y < -1.5)) {
        return lit;
    }
    let dims = vec2<f32>(textureDimensions(second_tex));
    let mirrored = textureSampleLevel(second_tex, second_samp, v.clip.xy / dims, 0.0);
    let to_eye = normalize(u.eye.xyz - v.world);
    let amount = mix(GLOSS_DOWN, GLOSS_GRAZING, pow(1.0 - clamp(to_eye.y, 0.0, 1.0), 3.0)) * u.gloss.w;
    return mix(lit, mirrored.rgb, amount * clamp(mirrored.a, 0.0, 1.0));
}

// Signals on a circuit board (ours, Enhanced and Ultra): a lemming standing
// on a trace sends a faint pulse of light from its feet along the traces
// joined to it, round their corners (`crate::signals`). Unlit, so that the
// glowing traps stay what catches the eye.

/// How fast a signal runs, in texels a second (as `crate::signals`), how
/// long its fading tail is, in texels, and its light.
const SIGNAL_SPEED: f32 = 45.0;
const SIGNAL_TAIL: f32 = 14.0;
const SIGNAL_LIGHT: vec3<f32> = vec3(0.04, 0.2, 0.06);

/// The light of a signal a lemming sent (`crate::signals`) at `world` on a
/// circuit board, a texel spanning `texel` pixels: its front's arrival at
/// the field texel there (`crate::signals::SignalField::arrivals`), lit
/// for a short tail after, or longer round where the signal set off.
fn sent_signal(world: vec3<f32>, texel: f32) -> vec3<f32> {
    if (u.gloss.z <= 0.5) {
        return vec3(0.0);
    }
    let dims = vec2<i32>(textureDimensions(field_tex));
    let at = vec2<i32>(floor(world.xz * u.gloss.y));
    if (any(at < vec2(0)) || any(at >= dims)) {
        return vec3(0.0);
    }
    let b = round(textureLoad(field_tex, at, 0) * 255.0);
    // When the front gets here (milliseconds, wrapping at 2^24), and how long
    // it stays lit here (4 ms steps; 0 for a front passing).
    let arrival = b.r + b.g * 256.0 + b.b * 65536.0;
    if (arrival <= 0.0) {
        return vec3(0.0);
    }
    let lit_ms = max(SIGNAL_TAIL / SIGNAL_SPEED * 1000.0, b.a * 4.0);
    let since = (u.sky.w * 1000.0) % 16777216.0 - arrival;
    if (since < 0.0 || since > lit_ms) {
        return vec3(0.0);
    }
    let near = 1.0 - smoothstep(3.0, 8.0, texel);
    return SIGNAL_LIGHT * (1.0 - since / lit_ms) * near;
}

// Gentle snow (ours, Enhanced and Ultra, the icy levels): sparse soft flakes
// drift down slowly, swaying a little. Each flake has its own place in the
// world, in a box that wraps around the camera: moving, the camera passes
// among them; one leaving the box comes back on its far side, where flakes
// fade out. Drawn as billboards, hidden by what is nearer.

/// How many flakes there are; the box's size, in units; how fast they fall,
/// in units a second; how far they sway; their radius, in units.
const SNOW_FLAKES: u32 = 3000u;
const SNOW_BOX: f32 = 16.0;
const SNOW_FALL: f32 = 0.45;
const SNOW_SWAY: f32 = 0.12;
const SNOW_SIZE: f32 = 0.025;

struct SnowOut {
    @builtin(position) clip: vec4<f32>,
    // Where on the flake, -1 to 1 across; and its opacity.
    @location(0) at: vec2<f32>,
    @location(1) alpha: f32,
};

@vertex
fn vs_snow(@builtin(vertex_index) i: u32, @builtin(instance_index) n: u32) -> SnowOut {
    let t = u.sky.w;
    let k = f32(n);
    let seed = vec3(hash2(vec2(k, 1.0)), hash2(vec2(k, 2.0)), hash2(vec2(k, 3.0)));
    // Its place in the world, falling and swaying, wrapped into the box
    // around the camera.
    var p = seed * SNOW_BOX;
    p.y -= t * SNOW_FALL * (0.8 + 0.4 * hash2(vec2(k, 4.0)));
    let phase = hash2(vec2(k, 5.0)) * 6.283;
    p.x += sin(t * 0.7 + phase) * SNOW_SWAY;
    p.z += cos(t * 0.5 + phase) * SNOW_SWAY;
    let rel = (p - u.eye.xyz + SNOW_BOX * 0.5) - floor((p - u.eye.xyz + SNOW_BOX * 0.5) / SNOW_BOX) * SNOW_BOX - SNOW_BOX * 0.5;
    let world = u.eye.xyz + rel;
    // Faded out towards the box's edge and very close to the eye.
    let reach = max(abs(rel.x), max(abs(rel.y), abs(rel.z))) / (SNOW_BOX * 0.5);
    var alpha = (1.0 - smoothstep(0.6, 1.0, reach)) * smoothstep(0.15, 0.5, length(rel));
    // At least about a pixel across, fainter for it, so far flakes do not
    // flicker away.
    let c = u.view_proj * vec4(world, 1.0);
    let c_up = u.view_proj * vec4(world + vec3(0.0, 1.0, 0.0), 1.0);
    let per_unit = abs(c_up.y / c_up.w - c.y / c.w) * 200.0 * u.sky.z;
    let size = SNOW_SIZE * (0.7 + 0.6 * hash2(vec2(k, 6.0)));
    let radius = max(size, 0.9 / max(per_unit, 1e-3));
    alpha *= min(1.0, size / radius) * 0.8 * u.snow.y;
    let corner = vec2(f32((0x32u >> i) & 1u), f32((0x2Cu >> i) & 1u)) * 2.0 - 1.0;
    let at = world + (u.right.xyz * corner.x + vec3(0.0, corner.y, 0.0)) * radius;
    var o: SnowOut;
    o.clip = u.view_proj * vec4(at, 1.0);
    o.at = corner;
    o.alpha = alpha;
    return o;
}

@fragment
fn fs_snow(v: SnowOut) -> @location(0) vec4<f32> {
    let a = (1.0 - smoothstep(0.4, 1.0, length(v.at))) * v.alpha;
    if (a <= 0.0) {
        discard;
    }
    return vec4(vec3(0.95, 0.97, 1.0), a);
}

// Space seen through a spaceship's windows (ours, Enhanced and Ultra;
// `glow::window_tile`): stars in three layers, faint nebulae and a banded
// planet lit from one side, all infinitely far off (they depend only on the
// direction looked in), turning slowly about the vertical as if the ship
// were.

/// How fast the view turns, radians a second; where the planet is, its
/// angular radius, and where its sun is.
const SPACE_TURN: f32 = 0.004;
const PLANET_AT: vec3<f32> = vec3(0.55, 0.12, -0.82);
const PLANET_RADIUS: f32 = 0.24;
const PLANET_SUN: vec3<f32> = vec3(-0.75, 0.35, 0.2);
const PLANET_LIGHT: vec3<f32> = vec3(0.9, 0.55, 0.32);
const PLANET_DARK: vec3<f32> = vec3(0.35, 0.16, 0.1);
const PLANET_AIR: vec3<f32> = vec3(0.35, 0.6, 1.0);

/// Tiles in the block texture, stacked one above the other
/// (`level_mesh::TILES`).
const TILES: f32 = 100.0;

/// How much of the direction looked in one pixel spans at the middle of the
/// view: between the rays through the middle and a pixel above it.
fn pixel_angle() -> f32 {
    let height = u.sky.z * 400.0;
    let a = u.inv_view_proj * vec4(0.0, 0.0, 0.5, 1.0);
    let b = u.inv_view_proj * vec4(0.0, 2.0 / height, 0.5, 1.0);
    let da = normalize(a.xyz / a.w - u.eye.xyz);
    let db = normalize(b.xyz / b.w - u.eye.xyz);
    return length(db - da);
}

/// The view of space in direction `d`, a pixel spanning `px` of direction.
fn space(d0: vec3<f32>, px: f32) -> vec3<f32> {
    let t = u.sky.w;
    let a = t * SPACE_TURN;
    let d = vec3(d0.x * cos(a) - d0.z * sin(a), d0.y, d0.x * sin(a) + d0.z * cos(a));
    // Coordinates on the face of a cube the direction points at.
    let m = abs(d);
    var uv: vec2<f32>;
    var face: f32;
    if (m.x >= m.y && m.x >= m.z) {
        uv = d.yz / m.x;
        face = select(1.0, 2.0, d.x < 0.0);
    } else if (m.y >= m.z) {
        uv = d.xz / m.y;
        face = select(3.0, 4.0, d.y < 0.0);
    } else {
        uv = d.xy / m.z;
        face = select(5.0, 6.0, d.z < 0.0);
    }
    let uv_px = px / max(m.x, max(m.y, m.z));
    // Faint nebulae.
    let haze = value_noise(uv * 2.5 + face * 7.0) * value_noise(uv * 5.0 - face * 3.0);
    var c = vec3(0.004, 0.005, 0.012) + mix(vec3(0.03, 0.01, 0.05), vec3(0.01, 0.03, 0.06), haze)
        * smoothstep(0.15, 0.6, haze);
    // Stars: in each layer a few cells of a grid hold one, at its own spot,
    // about a pixel across, twinkling a little.
    var stars = vec3(0.0);
    for (var l = 0; l < 3; l++) {
        let n = 24.0 * pow(2.2, f32(l));
        let q = uv * n;
        let cell = floor(q);
        let key = cell + vec2(face * 131.0, f32(l) * 977.0);
        if (hash2(key) > 0.22) {
            continue;
        }
        let spot = vec2(hash2(key + 3.1), hash2(key + 7.7)) * 0.8 + 0.1;
        let dist = length(q - cell - spot) / n / max(uv_px, 1e-5);
        let size = 0.7 + 0.8 * hash2(key + 11.3) / (1.0 + f32(l));
        let twinkle = 0.85 + 0.15 * sin(t * (1.0 + 3.0 * hash2(key + 5.5)) + hash2(key) * 40.0);
        let tint = mix(vec3(0.7, 0.8, 1.0), vec3(1.0, 0.85, 0.7), hash2(key + 13.0));
        stars += tint * exp(-dist * dist / (size * size)) * twinkle / (1.0 + 0.6 * f32(l));
    }
    // Bright points: lifted above white with glow (Ultra), and as far as
    // white without.
    c += stars * (1.8 + 0.7 * u.post.x);
    // The planet, in front of the stars.
    let p = normalize(PLANET_AT);
    let cos_r = cos(PLANET_RADIUS);
    let along = dot(d, p);
    let rim = smoothstep(cos_r - 0.004, cos_r + 0.002, along);
    let glow_rim = smoothstep(cos_r - 0.05, cos_r, along) * (1.0 - rim);
    if (along > cos_r - 0.05) {
        // Where on the disc, -1 to 1 from its middle, and its surface's
        // normal there.
        let off = (d - p * along) / sin(PLANET_RADIUS);
        let r2 = min(dot(off, off), 1.0);
        let normal = normalize(off + p * sqrt(1.0 - r2));
        let sun = max(dot(normal, normalize(PLANET_SUN)), 0.0);
        let lat = dot(normal, vec3(0.0, 1.0, 0.0));
        let bands = 0.5 + 0.5 * sin(lat * 18.0 + value_noise(vec2(lat * 9.0, dot(normal.xz, vec2(1.0, 0.4)) * 4.0)) * 3.0);
        let ground = mix(PLANET_DARK, PLANET_LIGHT, bands);
        let lit = ground * (0.04 + 0.96 * smoothstep(0.0, 0.6, sun));
        let air = PLANET_AIR * pow(1.0 - sqrt(1.0 - r2), 3.0) * sun * 0.6;
        c = mix(c, lit + air, rim);
        c += PLANET_AIR * glow_rim * 0.15 * max(dot(normalize(off), normalize(PLANET_SUN - p * dot(PLANET_SUN, p))), 0.0);
    }
    return c;
}

// Marks the lemmings leave on an icy level (ours, Ultra mode;
// `crate::marks`): footprints and slides pressed into snow, slides
// scratched into ice. Snow is shaded as if pushed aside: each mark a trough,
// its walls lit from one side and shaded on the other, with a raised ridge
// of the snow it displaced around it.

/// How much a trough's bottom darkens snow, and the blue-grey it takes on;
/// how much its walls brighten or darken facing towards or away from the
/// light; how much its ridge brightens; how far out, in field texels, the
/// ridge lies; how much a slide's scratch lightens ice.
const PRINT_DARK: f32 = 0.22;
const PRINT_TINT: vec3<f32> = vec3(0.8, 0.88, 1.0);
const WALL_LIGHT: f32 = 0.45;
const RIDGE_LIGHT: f32 = 0.18;
const RIDGE_OUT: f32 = 3.0;
const SCRATCH_LIGHT: f32 = 0.35;
/// How deep a slide's groove in snow is, against a footprint's 1.
const GROOVE: f32 = 0.8;
/// Where the light comes from across the ground (x and z), for the walls.
const MARK_LIGHT: vec2<f32> = vec2(-0.6, -0.8);
/// How far from a mark's surface, in units, it still shows.
const MARK_REACH: f32 = 0.08;
/// Steps per unit a mark's surface height is counted in
/// (`crate::marks::HEIGHT_STEPS`).
const MARK_HEIGHT_STEPS: f32 = 8.0;

/// How deep the snow is pressed at field coordinates `uv` (0–1), and the
/// slide scratch there.
fn pressed(uv: vec2<f32>) -> vec2<f32> {
    let m = textureSampleLevel(field_tex, second_samp, uv, 0.0);
    return vec2(max(m.r, m.g * GROOVE), m.g);
}

/// `lit`, the colour of an upward face at `world`, its texel `srgb` (sRGB),
/// with the marks there: troughs where it is snow (near white, barely
/// tinted), slide scratches elsewhere.
fn marked(lit: vec3<f32>, world: vec3<f32>, srgb: vec3<f32>) -> vec3<f32> {
    let dims = vec2<f32>(textureDimensions(field_tex));
    let uv = world.xz * u.snow.z / dims;
    let px = 1.0 / dims;
    let m = textureSampleLevel(field_tex, second_samp, uv, 0.0);
    let height = round(m.b * 255.0) / MARK_HEIGHT_STEPS;
    if (m.b == 0.0 || abs(world.y - height) > MARK_REACH) {
        return lit;
    }
    let here = pressed(uv);
    // The snow pushed out of the way, piled round the trough.
    let r = px * RIDGE_OUT;
    let ring = 0.25 * (pressed(uv + vec2(r.x, 0.0)).x + pressed(uv - vec2(r.x, 0.0)).x
        + pressed(uv + vec2(0.0, r.y)).x + pressed(uv - vec2(0.0, r.y)).x);
    if (here.x + ring + here.y < 0.004) {
        return lit;
    }
    let low = min(srgb.r, min(srgb.g, srgb.b));
    let spread = max(srgb.r, max(srgb.g, srgb.b)) - low;
    let snow = smoothstep(0.72, 0.85, low) * (1.0 - smoothstep(0.08, 0.18, spread));
    // The trough's slope, from its depth either side: a wall facing the
    // light is lit, one facing away shaded.
    let e = px * 1.5;
    let slope = vec2(
        pressed(uv + vec2(e.x, 0.0)).x - pressed(uv - vec2(e.x, 0.0)).x,
        pressed(uv + vec2(0.0, e.y)).x - pressed(uv - vec2(0.0, e.y)).x,
    );
    let wall = clamp(dot(slope, normalize(MARK_LIGHT)), -1.0, 1.0) * WALL_LIGHT;
    let ridge = max(ring - here.x, 0.0) * RIDGE_LIGHT;
    let bottom = mix(vec3(1.0), PRINT_TINT * (1.0 - PRINT_DARK), here.x);
    var out = mix(lit, lit * bottom * (1.0 + wall + ridge), snow);
    let top = max(out.r, max(out.g, out.b));
    out = mix(out, vec3(max(top, 0.9)), here.y * SCRATCH_LIGHT * (1.0 - snow));
    return out;
}

// Snow lying on the level (ours, Ultra mode; `scene_build::SNOW_DEPTH`): a
// layer the lemmings' marks press down into troughs, heaping what they push
// aside into a low ridge alongside, lit by its slopes.

/// How high the ridge rises, against the snow's depth there; where the light
/// comes from; how strongly slopes turn towards or away from it shade the
/// snow; how dark a trough's bottom is.
const RIDGE_RISE: f32 = 0.6;
const SNOW_LIGHT: vec3<f32> = vec3(-0.45, 0.8, -0.4);
const SLOPE_SHADE: f32 = 2.2;
const TROUGH_DARK: f32 = 0.32;
/// How far out, in field texels, the snow pushed aside heaps up.
const SNOW_RIDGE_OUT: f32 = 5.0;
/// How far, in units, the ground under the snow may lie from a mark's
/// surface and still take it: a slope's height changes across a mark.
const SNOW_MARK_REACH: f32 = 0.3;

/// How far the snow lying at `world`, `depth` deep over ground at height
/// `ground`, is moved up (down where pressed), and how far it is pressed,
/// 0–1 of its depth.
fn snow_pressed(world: vec3<f32>, depth: f32, ground: f32) -> vec2<f32> {
    if (u.snow.z <= 0.0) {
        return vec2(0.0);
    }
    let dims = vec2<f32>(textureDimensions(field_tex));
    let uv = world.xz * u.snow.z / dims;
    let m = textureSampleLevel(field_tex, second_samp, uv, 0.0);
    let floor_y = round(m.b * 255.0) / MARK_HEIGHT_STEPS;
    if (m.b == 0.0 || abs(ground - floor_y) > SNOW_MARK_REACH) {
        return vec2(0.0);
    }
    let here = pressed(uv).x;
    let r = SNOW_RIDGE_OUT / dims;
    let ring = 0.25 * (pressed(uv + vec2(r.x, 0.0)).x + pressed(uv - vec2(r.x, 0.0)).x
        + pressed(uv + vec2(0.0, r.y)).x + pressed(uv - vec2(0.0, r.y)).x);
    let rise = max(ring - here, 0.0) * RIDGE_RISE;
    return vec2((rise - here) * depth, here);
}

/// `lit`, the colour of snow lying on the level facing `facing`, which faced
/// `untouched` before the lemmings' marks, pressed `pressed` of its depth
/// there, lit by how its slope turns from the untouched one.
fn snow_lit(lit: vec3<f32>, facing: vec3<f32>, untouched: vec3<f32>, pressed: f32) -> vec3<f32> {
    let n = facing * select(1.0, -1.0, facing.y < 0.0);
    let n0 = untouched * select(1.0, -1.0, untouched.y < 0.0);
    let l = normalize(SNOW_LIGHT);
    let shade = clamp(1.0 + SLOPE_SHADE * (dot(n, l) - dot(n0, l)), 0.45, 1.25);
    let bottom = mix(vec3(1.0), PRINT_TINT * (1.0 - TROUGH_DARK), pressed);
    return lit * shade * bottom;
}
