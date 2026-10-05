// Binary coverage on an integer logical grid. User image sampling is separate.
struct Grid { screen:vec2<f32>, scale:f32, padding:f32 }
@group(0) @binding(0) var<uniform> grid:Grid;
struct Output {
 @builtin(position) position:vec4<f32>, @location(0) @interpolate(flat) origin:vec2<f32>,
 @location(1) @interpolate(flat) size:vec2<f32>, @location(2) @interpolate(flat) width:f32,
 @location(3) @interpolate(flat) kind:u32, @location(4) @interpolate(flat) stroke:vec4<f32>,
 @location(5) @interpolate(flat) fill:vec4<f32>, @location(6) @interpolate(flat) mapping:vec4<f32>,
 @location(7) @interpolate(flat) bits0:vec4<u32>, @location(8) @interpolate(flat) bits1:vec4<u32>,
 @location(9) @interpolate(flat) axes:vec4<f32>,
};
fn screen(p:vec2<f32>)->vec2<f32> { return (p*vec2(1.,-1.)+1.)*grid.screen*0.5/grid.scale; }
@vertex fn vertex(@builtin(vertex_index) index:u32,@location(0) p0:vec2<f32>,@location(1) p1:vec2<f32>,@location(2) p2:vec2<f32>,@location(3) p3:vec2<f32>,@location(4) size:vec2<f32>,@location(5) width:f32,@location(6) kind:u32,@location(7) stroke:vec4<f32>,@location(8) fill:vec4<f32>,@location(9) mapping:vec4<f32>,@location(10) bits0:vec4<u32>,@location(11) bits1:vec4<u32>) -> Output {
 let a=screen(p0);let b=screen(p1);let c=screen(p2);let d=screen(p3);
 // Axis-aligned raster envelope ensures an entire logical pixel samples once.
 let lo=floor(min(min(a,b),min(c,d)));let hi=ceil(max(max(a,b),max(c,d)));
 let indices=array<u32,6>(0,1,2,2,1,3);let corners=array<vec2<f32>,4>(lo,vec2(lo.x,hi.y),vec2(hi.x,lo.y),hi);
 let point=corners[indices[index]]*grid.scale/grid.screen*2.-1.;
 var o:Output;o.position=vec4(point*vec2(1.,-1.),0.,1.);o.origin=a;o.axes=vec4(c-a,b-a);o.size=size/grid.scale;o.width=width/grid.scale;o.kind=kind;o.stroke=stroke;o.fill=fill;o.mapping=mapping;o.bits0=bits0;o.bits1=bits1;return o;
}
fn cross(a:vec2<f32>,b:vec2<f32>)->f32 {return a.x*b.y-a.y*b.x;}
fn prior_coverage(position:vec2<f32>,endpoints:vec4<f32>,width:f32)->f32 {
 let a=endpoints.xy/grid.scale;let b=endpoints.zw/grid.scale;let v=b-a;let length=max(length(v),1e-10);let direction=v/length;
 let p=position-(a+b)*0.5;
 return select(0.,1.,abs(dot(p,direction))<=length*0.5+width*0.5 && abs(cross(p,direction))<=width*0.5);
}
fn joined_alpha(o:Output,position:vec2<f32>,coverage:f32)->f32 {
 if o.bits0.x==0u {return o.stroke.a*coverage;}
 let prior=prior_coverage(position,o.mapping,o.width);
 let existing=o.stroke.a*prior;let desired=o.stroke.a*max(prior,coverage);
 return max(desired-existing,0.)/max(1.-existing,1e-7);
}
@fragment fn fragment(o:Output) -> @location(0) vec4<f32> {
 let position=floor(o.position.xy/grid.scale)+0.5;
 let delta=position-o.origin;let x=o.axes.xy;let y=o.axes.zw;let det=cross(x,y);
 if abs(det)<1e-12 {discard;}
 let uv=vec2(cross(delta,y),cross(x,delta))/det;
 if any(uv<vec2(0.)) || any(uv>=vec2(1.)) {discard;}
 if o.kind==7u { return o.stroke; }
 if o.kind==6u {
  let p=clamp(vec2<u32>((o.mapping.xy+uv*o.mapping.zw)*16.),vec2<u32>(0),vec2<u32>(15));let row=p.y/2u;
  var bits=o.bits0[min(row,3u)];if row>=4u { bits=o.bits1[row-4u]; }
  let mask=(bits>>((p.y%2u)*16u+15u-p.x))&1u;return vec4(o.stroke.rgb,o.stroke.a*f32(mask));
 }
 if o.kind==4u {
  return vec4(o.stroke.rgb,joined_alpha(o,position,select(0.,1.,uv.x+uv.y<=1.)));
 }
 let margin=o.width*0.5+1.;let p=(uv-0.5)*(o.size+vec2(margin*2.));let half=o.size*0.5;
 if o.kind==3u {
  let coverage=select(0.,1.,abs(p.x)<=half.x+o.width*0.5 && abs(p.y)<=o.width*0.5);
  return vec4(o.stroke.rgb,joined_alpha(o,position,coverage));
 }
 let d=abs(p)-half;let distance=max(d.x,d.y);
 let stroke_alpha=o.stroke.a*select(0.,1.,abs(distance)<=o.width*0.5);
 let fill_alpha=o.fill.a*select(0.,1.,distance<=0.)*(1.-stroke_alpha);
 let alpha=stroke_alpha+fill_alpha;
 return vec4((o.stroke.rgb*stroke_alpha+o.fill.rgb*fill_alpha)/max(alpha,0.000001),alpha*bitcast<f32>(o.bits0.x));
}
