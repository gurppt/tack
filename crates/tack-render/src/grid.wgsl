struct Background {
    grid: vec4<f32>,
    fill: vec4<f32>,
    dots: vec4<f32>,
    screen: vec4<f32>,
    major: vec4<f32>,
}
@group(0) @binding(0) var<uniform> background: Background;
@vertex fn vertex(@builtin(vertex_index) i:u32) -> @builtin(position) vec4<f32> {
    let p = array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    return vec4(p[i],0.,1.);
}
@fragment fn fragment(@builtin(position) p:vec4<f32>) -> @location(0) vec4<f32> {
    if background.screen.z > 0. {
        let grid = background.grid;
        if background.major.w > 0. {
            let d = abs(p.xy - floor(grid.xy + round((p.xy-grid.xy)/grid.z)*grid.z) - vec2(grid.w*0.5));
            let m = background.major;
            let md = abs(p.xy - floor(m.xy + round((p.xy-m.xy)/m.z)*m.z) - vec2(grid.w*0.5));
            if min(md.x, md.y) < grid.w*0.5 {return mix(background.fill, background.dots, 0.85);}
            if min(d.x, d.y) < grid.w*0.5 {return mix(background.fill, background.dots, 0.4);}
            return background.fill;
        }
        let nearest = floor(grid.xy + round((p.xy-grid.xy)/grid.z)*grid.z);
        if all(p.xy >= nearest) && all(p.xy < nearest+vec2(grid.w)) { return background.dots; }
    }
    return background.fill;
}
