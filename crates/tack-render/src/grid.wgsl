struct Background {
    grid: vec4<f32>,
    start: vec4<f32>,
    end: vec4<f32>,
    dots: vec4<f32>,
    screen: vec4<f32>,
}
@group(0) @binding(0) var<uniform> background: Background;
@vertex fn vertex(@builtin(vertex_index) i:u32) -> @builtin(position) vec4<f32> {
    let p = array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    return vec4(p[i],0.,1.);
}
@fragment fn fragment(@builtin(position) p:vec4<f32>) -> @location(0) vec4<f32> {
    let t = clamp(p.y / max(background.screen.y, 1.), 0., 1.);
    if background.screen.z > 0. {
        let grid = background.grid;
        let nearest = floor(grid.xy + round((p.xy-grid.xy)/grid.z)*grid.z);
        if all(p.xy >= nearest) && all(p.xy < nearest+vec2(grid.w)) { return background.dots; }
    }
    return mix(background.start, background.end, t);
}
