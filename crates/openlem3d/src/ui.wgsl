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
};

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) bounds: vec4<f32>,
    // The texture coordinates at the rectangle's corners.
    @location(3) @interpolate(flat) uv_rect: vec4<f32>,
};

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32, q: Quad) -> Out {
    // Top right, top left, bottom right, bottom left: split along the
    // diagonal bevy_ui used, so pixels exactly between two texels pick the
    // same one.
    let corner = vec2<f32>(f32(1u - (i & 1u)), f32(i >> 1u));
    var out: Out;
    out.pos = vec4<f32>(mix(q.rect.xy, q.rect.zw, corner), 0.0, 1.0);
    out.uv = mix(q.uv.xy, q.uv.zw, corner);
    out.color = q.color;
    out.bounds = q.bounds;
    out.uv_rect = q.uv;
    return out;
}

// Where to sample (with a linear sampler) so that pixel art scaled by any
// amount keeps square texels: each screen pixel takes the texel it lies in,
// and only a pixel straddling two texels blends them, by how much of it lies
// in each. At whole-number scales that is exactly the nearest texel. Never
// beyond the picture's own texels, so neighbours in a sheet don't bleed in.
fn sharp_uv(uv: vec2<f32>, uv_rect: vec4<f32>) -> vec2<f32> {
    let size = vec2<f32>(textureDimensions(image));
    let texel = uv * size;
    let pixels_per_texel = 1.0 / max(fwidth(texel), vec2<f32>(1e-6));
    let seam = floor(texel + 0.5);
    let t = seam + clamp((texel - seam) * pixels_per_texel, vec2<f32>(-0.5), vec2<f32>(0.5));
    let lo = (min(uv_rect.xy, uv_rect.zw) * size + 0.5) / size;
    let hi = (max(uv_rect.xy, uv_rect.zw) * size - 0.5) / size;
    return clamp(t / size, min(lo, hi), max(lo, hi));
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    // Signed distance from the pixel's centre to the rectangle's edge.
    let half = (in.bounds.zw - in.bounds.xy) * 0.5;
    let q = abs(in.pos.xy - (in.bounds.xy + half)) - half;
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
    let color = textureSample(image, image_sampler, sharp_uv(in.uv, in.uv_rect)) * in.color;
    return vec4<f32>(color.rgb, color.a * saturate(0.5 - d));
}
