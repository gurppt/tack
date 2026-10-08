struct Grid { screen:vec2<f32>, scale:f32, padding:f32 }
@group(0) @binding(0) var<uniform> grid:Grid;
struct Output {
 @builtin(position) position:vec4<f32>,
 @location(0) @interpolate(flat) origin:vec2<f32>,
 @location(1) @interpolate(flat) axes:vec4<f32>,
 @location(2) @interpolate(flat) color:vec4<f32>,
 @location(3) @interpolate(flat) width:f32,
 @location(4) @interpolate(flat) edge:u32,
}
@vertex fn vertex(@builtin(vertex_index) vertex_index:u32, @location(0) origin:vec2<f32>, @location(1) axes:vec4<f32>, @location(2) color:vec4<f32>, @location(3) width:f32)->Output {
 let edge=vertex_index/6u;
 let size=vec2(length(axes.xy),length(axes.zw));
 let half_width=max(round(width),1.)*0.5;
 var lo=vec2(-half_width);var hi=size+half_width;
 if edge==0u {hi.y=half_width;}
 if edge==1u {lo.y=size.y-half_width;}
 if edge==2u {hi.x=half_width;}
 if edge==3u {lo.x=size.x-half_width;}
 // Expand each oriented strip by one logical pixel to cover the snapped
 // sample grid. The fragment test removes this guard without antialiasing.
 lo=lo-1.;hi=hi+1.;
 let corner=array<u32,6>(0u,1u,2u,2u,1u,3u)[vertex_index%6u];
 let local=vec2(select(lo.x,hi.x,corner>=2u),select(lo.y,hi.y,(corner%2u)==1u));
 let screen=origin+axes.xy*(local.x/size.x)+axes.zw*(local.y/size.y);
 var o:Output;o.position=vec4(screen.x*grid.scale/grid.screen.x*2.-1.,1.-screen.y*grid.scale/grid.screen.y*2.,0.,1.);
 o.origin=origin;o.axes=axes;o.color=color;o.width=width;o.edge=edge;return o;
}
fn cross(a:vec2<f32>,b:vec2<f32>)->f32 {return a.x*b.y-a.y*b.x;}
@fragment fn fragment(o:Output)->@location(0) vec4<f32> {
 let p=floor(o.position.xy/grid.scale)+0.5-o.origin;
 let det=cross(o.axes.xy,o.axes.zw);if abs(det)<1e-12 {discard;}
 let uv=vec2(cross(p,o.axes.zw),cross(o.axes.xy,p))/det;
 let size=vec2(length(o.axes.xy),length(o.axes.zw));
 let local=uv*size;
 let distance=min(local,size-local);
 let half_width=max(round(o.width),1.)*0.5;
 if any(distance < vec2(-half_width)) || all(distance >= vec2(half_width)) {discard;}
 // A corner belongs to just one edge, avoiding duplicate alpha blending.
 let edges=abs(vec4(local.y,local.y-size.y,local.x,local.x-size.x));
 var nearest=0u;for(var i=1u;i<4u;i=i+1u) {if edges[i]<edges[nearest] {nearest=i;}}
 if o.edge!=nearest {discard;}
 return o.color;
}
