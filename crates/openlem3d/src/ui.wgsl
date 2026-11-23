// The UI: one textured, tinted rectangle per instance, drawn as a triangle
// strip. Rectangles and texture coordinates arrive ready to use. A pixel
// only partly inside the rectangle is drawn as much as it is (as bevy_ui
// did), which shows where a picture doesn't fill whole pixels.

struct Quad {
    // Left, top, right, bottom in normalised device coordinates.
    @location(0) rect: vec4<f32>,
    // The texture coordinates at those corners.
    @location(1) uv: vec4<f32>,
    // Linear colour multiplied onto the texture.
    @location(2) color: vec4<f32>,
    // The rectangle in framebuffer pixels.
    @location(3) bounds: vec4<f32>,
    // The picture's size in texels (xy), and screen pixels per texel
    // across and down (zw).
    @location(4) texels: vec4<f32>,
};

struct Out {
    @builtin(position) pos: vec4<f32>,
    // Where in the picture, in texels.
    @location(0) texel: vec2<f32>,
    @location(1) color: vec4<f32>,
    // The rectangle's middle and half its size, in framebuffer pixels.
    @location(2) @interpolate(flat) middle: vec4<f32>,
    // The texture coordinates the picture's outermost texel centres lie at:
    // lowest (xy) and highest (zw).
    @location(3) @interpolate(flat) uv_clamp: vec4<f32>,
    // One over the picture's size in texels (xy), and screen pixels per
    // texel (zw).
    @location(4) @interpolate(flat) scale: vec4<f32>,
};

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;

// What is the same over the whole rectangle is worked out here, once a
// corner, rather than for every pixel: on a weak GPU (a Mali-G31) the
// per-pixel version was most of the frame's UI cost.
@vertex
fn vs(@builtin(vertex_index) i: u32, q: Quad) -> Out {
    // Top right, top left, bottom right, bottom left: split along the
    // diagonal bevy_ui used, so pixels exactly between two texels pick the
    // same one.
    let corner = vec2<f32>(f32(1u - (i & 1u)), f32(i >> 1u));
    let size = q.texels.xy;
    var out: Out;
    out.pos = vec4<f32>(mix(q.rect.xy, q.rect.zw, corner), 0.0, 1.0);
    out.texel = mix(q.uv.xy, q.uv.zw, corner) * size;
    out.color = q.color;
    let half = (q.bounds.zw - q.bounds.xy) * 0.5;
    out.middle = vec4<f32>(q.bounds.xy + half, half);
    let lo = (min(q.uv.xy, q.uv.zw) * size + 0.5) / size;
    let hi = (max(q.uv.xy, q.uv.zw) * size - 0.5) / size;
    out.uv_clamp = vec4<f32>(min(lo, hi), max(lo, hi));
    out.scale = vec4<f32>(1.0 / size, q.texels.zw);
    return out;
}

// Where to sample (with a linear sampler) so that pixel art scaled by any
// amount keeps square texels: each screen pixel takes the texel it lies in,
// and only a pixel straddling two texels blends them, by how much of it lies
// in each. At whole-number scales that is exactly the nearest texel. Never
// beyond the picture's own texels, so neighbours in a sheet don't bleed in.
fn sharp_uv(texel: vec2<f32>, uv_clamp: vec4<f32>, scale: vec4<f32>) -> vec2<f32> {
    let seam = floor(texel + 0.5);
    let t = seam + clamp((texel - seam) * scale.zw, vec2<f32>(-0.5), vec2<f32>(0.5));
    return clamp(t * scale.xy, uv_clamp.xy, uv_clamp.zw);
}

/// Screen pixels per texel from which a picture takes its nearest texels.
const NEAREST_FROM: f32 = 3.0;

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    // Large enough, each texel spans pixels enough that its nearest texel
    // looks the same, for far less work; nor need the edges be drawn
    // partly then.
    var uv: vec2<f32>;
    var cover = vec2<f32>(1.0);
    if (min(in.scale.z, in.scale.w) >= NEAREST_FROM) {
        uv = clamp((floor(in.texel) + 0.5) * in.scale.xy, in.uv_clamp.xy, in.uv_clamp.zw);
    } else {
        uv = sharp_uv(in.texel, in.uv_clamp, in.scale);
        // How much of the pixel the rectangle covers: across times down.
        let q = abs(in.pos.xy - in.middle.xy) - in.middle.zw;
        cover = saturate(0.5 - q);
    }
    let color = textureSample(image, image_sampler, uv) * in.color;
    let a = color.a * cover.x * cover.y;
    if (a <= 0.0) {
        discard;
    }
    return vec4<f32>(color.rgb, a);
}
