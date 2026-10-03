struct Output { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32> }
@vertex fn vertex(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>) -> Output {
    var out: Output; out.position = vec4<f32>(position,0.0,1.0); out.color=color; return out;
}
@fragment fn fragment(in: Output) -> @location(0) vec4<f32> { return in.color; }
