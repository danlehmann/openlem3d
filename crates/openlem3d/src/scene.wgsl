// Scene shaders: palette-textured unlit geometry, and the panned sky backdrop.

struct Uniforms {
    view_proj: mat4x4<f32>,
    // x: sky column (in sky texels) at the left edge of the screen,
    // y: horizon row in framebuffer pixels,
    // z: framebuffer pixels per original-screen pixel (height / 480),
    // w: time in seconds.
    sky: vec4<f32>,
    // Camera right vector (xyz); billboards are spanned by it and +Y.
    right: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // x: brightness, y: 1 if palette index 0 is transparent.
    @location(2) params: vec2<f32>,
    // Billboard corner offset from `pos`, in world units along the camera
    // right vector (x) and up (y). Zero for fixed geometry.
    @location(3) offset: vec2<f32>,
    // x: animation frame count (<= 1: static), y: uv distance between frames.
    @location(4) anim: vec2<f32>,
};

/// Animation frames per second of animated sprites (unverified).
const ANIM_FPS: f32 = 8.0;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) params: vec2<f32>,
};

@vertex
fn vs(v: VIn) -> VOut {
    var o: VOut;
    let world = v.pos + u.right.xyz * v.offset.x + vec3(0.0, v.offset.y, 0.0);
    o.clip = u.view_proj * vec4(world, 1.0);
    var frame = 0.0;
    if (v.anim.x > 1.0) {
        frame = floor(u.sky.w * ANIM_FPS) % v.anim.x;
    }
    o.uv = v.uv + vec2(0.0, frame * v.anim.y);
    o.params = v.params;
    return o;
}

@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, v.uv);
    if (v.params.y > 0.5 && c.a < 0.5) {
        discard;
    }
    return vec4(c.rgb * v.params.x, 1.0);
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
    let col = u.sky.x + v.clip.x / (2.0 * u.sky.z);
    let row = clamp(v.clip.y / u.sky.y, 0.0, 1.0) * dims.y;
    let uv = vec2(col / dims.x, min(row, dims.y - 0.5) / dims.y);
    return vec4(textureSampleLevel(tex, samp, uv, 0.0).rgb, 1.0);
}
