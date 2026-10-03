struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
};
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;

@vertex
fn vertex(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) opacity: f32) -> VertexOut {
    var output: VertexOut;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.uv = uv;
    output.opacity = opacity;
    return output;
}

@fragment
fn fragment(input: VertexOut) -> @location(0) vec4<f32> {
    let color = textureSample(image, image_sampler, input.uv);
    return vec4<f32>(color.rgb, color.a * input.opacity);
}
