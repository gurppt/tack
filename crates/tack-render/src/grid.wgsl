@group(0) @binding(0) var<uniform> grid: vec4<f32>;
@vertex fn vertex(@builtin(vertex_index) i:u32) -> @builtin(position) vec4<f32> {
    let p = array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    return vec4(p[i],0.,1.);
}
@fragment fn fragment(@builtin(position) p:vec4<f32>) -> @location(0) vec4<f32> {
    let nearest = floor(grid.xy + round((p.xy-grid.xy)/grid.z)*grid.z);
    if all(p.xy >= nearest) && all(p.xy < nearest+vec2(grid.w)) { return vec4(0.085,0.095,0.11,1.); }
    discard;
}
