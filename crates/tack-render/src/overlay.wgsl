struct Grid { screen:vec2<f32>, scale:f32, padding:f32 }
@group(0) @binding(0) var<uniform> grid:Grid;
struct Output {
 @builtin(position) position:vec4<f32>, @location(0) @interpolate(flat) color:vec4<f32>,
 @location(1) @interpolate(flat) origin:vec2<f32>, @location(2) @interpolate(flat) axes:vec4<f32>,
 @location(3) @interpolate(flat) bits0:vec4<u32>, @location(4) @interpolate(flat) bits1:vec4<u32>,
 @location(5) @interpolate(flat) bitmap:u32,
}
@vertex fn vertex(@location(0) position:vec2<f32>, @location(1) color:vec4<f32>, @location(2) bits0:vec4<u32>, @location(3) bits1:vec4<u32>, @location(4) bitmap:u32, @location(5) origin:vec2<f32>, @location(6) axes:vec4<f32>) -> Output {
 var o:Output;o.position=vec4(position,0.,1.);o.color=color;o.origin=origin;o.axes=axes;o.bits0=bits0;o.bits1=bits1;o.bitmap=bitmap;return o;
}
fn cross(a:vec2<f32>,b:vec2<f32>)->f32 {return a.x*b.y-a.y*b.x;}
@fragment fn fragment(o:Output)->@location(0) vec4<f32> {
 let p=floor(o.position.xy/grid.scale)+0.5-o.origin;
 let det=cross(o.axes.xy,o.axes.zw);if abs(det)<1e-12 {discard;}
 let uv=vec2(cross(p,o.axes.zw),cross(o.axes.xy,p))/det;
 if any(uv<vec2(0.)) || any(uv>=vec2(1.)) {discard;}
 if o.bitmap==2u {
  if (u32(floor(uv.x*length(o.axes.xy)))%8u)>=4u {discard;}
 }
 if o.bitmap==1u {
  let x=min(u32(uv.x*16.),15u);let y=min(u32(uv.y*16.),15u);
  var pair:u32;if y<8u {pair=o.bits0[y/2u];} else {pair=o.bits1[(y-8u)/2u];}
  let row=(pair>>((y%2u)*16u))&65535u;if (row&(1u<<(15u-x)))==0u {discard;}
 }
 return o.color;
}
