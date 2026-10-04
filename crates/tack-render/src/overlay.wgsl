struct Output {
    @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32>, @location(1) uv:vec2<f32>,
    @location(2) @interpolate(flat) bits0:vec4<u32>, @location(3) @interpolate(flat) bits1:vec4<u32>,
    @location(4) @interpolate(flat) bitmap:u32
}
@vertex fn vertex(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>, @location(2) uv:vec2<f32>, @location(3) bits0:vec4<u32>, @location(4) bits1:vec4<u32>, @location(5) bitmap:u32) -> Output {
    var out: Output; out.position = vec4<f32>(position,0.0,1.0); out.color=color; out.uv=uv;out.bits0=bits0;out.bits1=bits1;out.bitmap=bitmap;return out;
}
@fragment fn fragment(in: Output) -> @location(0) vec4<f32> {
    if in.bitmap != 0u {
        let x = min(u32(in.uv.x*16.),15u); let y = min(u32(in.uv.y*16.),15u);
        var pair:u32;
        if y<8u { pair=in.bits0[y/2u]; } else { pair=in.bits1[(y-8u)/2u]; }
        let row = (pair >> ((y%2u)*16u)) & 65535u;
        if (row & (1u << (15u-x))) == 0u { discard; }
    }
    return in.color;
}
