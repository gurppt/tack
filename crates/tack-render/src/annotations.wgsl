@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
struct Output {
 @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32>,
 @location(1) @interpolate(flat) size:vec2<f32>, @location(2) @interpolate(flat) width:f32,
 @location(3) @interpolate(flat) kind:u32, @location(4) @interpolate(flat) stroke:vec4<f32>,
 @location(5) @interpolate(flat) fill:vec4<f32>, @location(6) @interpolate(flat) atlas_uv:vec4<f32>,
 @location(7) @interpolate(flat) bits0:vec4<u32>, @location(8) @interpolate(flat) bits1:vec4<u32>,
};
@vertex fn vertex(@builtin(vertex_index) index:u32,@location(0) p0:vec2<f32>,@location(1) p1:vec2<f32>,@location(2) p2:vec2<f32>,@location(3) p3:vec2<f32>,@location(4) size:vec2<f32>,@location(5) width:f32,@location(6) kind:u32,@location(7) stroke:vec4<f32>,@location(8) fill:vec4<f32>,@location(9) atlas_uv:vec4<f32>,@location(10) bits0:vec4<u32>,@location(11) bits1:vec4<u32>) -> Output {
 let indices=array<u32,6>(0,1,2,2,1,3);let points=array<vec2<f32>,4>(p0,p1,p2,p3);let uvs=array<vec2<f32>,4>(vec2(0.,0.),vec2(0.,1.),vec2(1.,0.),vec2(1.,1.));let i=indices[index];
 var o:Output;o.position=vec4(points[i],0.,1.);o.uv=uvs[i];o.size=size;o.width=width;o.kind=kind;o.stroke=stroke;o.fill=fill;o.atlas_uv=atlas_uv;o.bits0=bits0;o.bits1=bits1;return o;
}
fn ellipse_distance(p:vec2<f32>,r:vec2<f32>)->f32 {
 var v=abs(p);var radii=max(r,vec2(1e-12));
 if radii.x<radii.y {v=v.yx;radii=radii.yx;}
 let scale=radii.x;let b=radii.y/scale;let x=v.x/scale;let y=v.y/scale;
 if b>0.999999 {return abs(length(vec2(x,y))-1.)*scale;}
 if y<=1e-7 {let q=min(x/(1.-b*b),1.);return length(vec2(x-q,y-b*sqrt(max(1.-q*q,0.))))*scale;}
 let inside=x*x+y*y/(b*b)<=1.;
 var lo=select(0.,b*(y-b),inside);var hi=select(x+b*y,0.,inside);
 for(var i=0u;i<32u;i+=1u) {let mid=(lo+hi)*0.5;let v=vec2(x/(mid+1.),b*y/(mid+b*b));if dot(v,v)>1. {lo=mid;} else {hi=mid;}}
 let q=(lo+hi)*0.5;return length(vec2(x-x/(q+1.),y-b*b*y/(q+b*b)))*scale;
}
fn prior_coverage(position:vec2<f32>,endpoints:vec4<f32>,width:f32)->f32 {
 let a=endpoints.xy;let b=endpoints.zw;let v=b-a;let t=clamp(dot(position-a,v)/max(dot(v,v),1e-20),0.,1.);
 let distance=length(position-(a+t*v));return 1.-smoothstep(width*0.5-0.75,width*0.5+0.75,distance);
}
fn joined_alpha(o:Output,coverage:f32)->f32 {
 if o.bits0.x==0u {return o.stroke.a*coverage;}
 let prior=prior_coverage(o.position.xy,o.atlas_uv,o.width);
 let existing=o.stroke.a*prior;let desired=o.stroke.a*max(prior,coverage);
 return max(desired-existing,0.)/max(1.-existing,1e-7);
}
@fragment fn fragment(o:Output) -> @location(0) vec4<f32> {
 if o.kind==7u { return o.stroke; }
 if o.kind==5u {
  let d=(textureSample(atlas,atlas_sampler,o.atlas_uv.xy+o.uv*o.atlas_uv.zw).r*255.-128.)/16.;
  let edge=max(fwidth(d)*0.6,0.01);return vec4(o.stroke.rgb,o.stroke.a*smoothstep(-edge,edge,d));
 }
 if o.kind==6u {
  let p=min(vec2<u32>((o.atlas_uv.xy+o.uv*o.atlas_uv.zw)*16.),vec2<u32>(15));let row=p.y/2u;
  var bits=o.bits0[min(row,3u)];if row>=4u { bits=o.bits1[row-4u]; }
  let mask=(bits>>((p.y%2u)*16u+15u-p.x))&1u;return vec4(o.stroke.rgb,o.stroke.a*f32(mask));
 }
 if o.kind==4u {
  let d=min(min(o.uv.x,o.uv.y),1.-o.uv.x-o.uv.y);let edge=max(fwidth(d)*0.6,0.00001);
  return vec4(o.stroke.rgb,joined_alpha(o,smoothstep(-edge,edge,d)));
 }
 // Quad includes one AA pixel and half the stroke beyond the shape.
 let margin=o.width*0.5+1.;let p=(o.uv-0.5)*(o.size+vec2(margin*2.));let half=o.size*0.5;
 var distance=0.;
 if o.kind==1u {
  let d=abs(p)-half;distance=length(max(d,vec2(0.)))+min(max(d.x,d.y),0.);
 } else if o.kind==2u {
  let inside=dot(p/max(half,vec2(1e-12)),p/max(half,vec2(1e-12)))<=1.;
  distance=ellipse_distance(p,half)*select(1.,-1.,inside);
 } else {
  let q=vec2(max(abs(p.x)-half.x,0.),p.y);distance=length(q);
 }
 if o.kind==3u {let coverage=1.-smoothstep(o.width*0.5-0.75,o.width*0.5+0.75,distance);return vec4(o.stroke.rgb,joined_alpha(o,coverage));}
 let stroke_alpha=o.stroke.a*(1.-smoothstep(o.width*0.5-0.75,o.width*0.5+0.75,abs(distance)));
 let fill_alpha=o.fill.a*(1.-smoothstep(-0.75,0.75,distance))*(1.-stroke_alpha);
 let alpha=stroke_alpha+fill_alpha;
 let opacity=select(1.,bitcast<f32>(o.bits0.x),o.kind==1u || o.kind==2u);
 return vec4((o.stroke.rgb*stroke_alpha+o.fill.rgb*fill_alpha)/max(alpha,0.000001),alpha*opacity);
}
