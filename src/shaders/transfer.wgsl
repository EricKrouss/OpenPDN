struct Params { count:u32, size:u32, luminosity:u32, unused:u32 }
@group(0) @binding(0) var<storage,read> source:array<u32>;
@group(0) @binding(1) var<storage,read> selection:array<u32>;
@group(0) @binding(2) var<storage,read> table:array<vec4<f32>>;
@group(0) @binding(3) var<uniform> params:Params;
@group(0) @binding(4) var<storage,read_write> output:array<u32>;
fn lookup(x:f32,c:u32)->f32 {let p=clamp(x,0.0,1.0)*f32(params.size-1u);let lo=u32(p);let hi=min(lo+1u,params.size-1u);return mix(table[lo][c],table[hi][c],p-f32(lo));}
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id:vec3<u32>) {
 let i=id.x;if i>=params.count {return;}let p=source[i];if selection[i]==0u {output[i]=p;return;}
 let rgb=vec3<f32>(f32(p&255u),f32((p>>8u)&255u),f32((p>>16u)&255u))/255.0;
 var color=vec3<f32>(lookup(rgb.r,0u),lookup(rgb.g,1u),lookup(rgb.b,2u));
 if params.luminosity!=0u {
    let weights=vec3<f32>(0.299,0.587,0.114);let gray=dot(rgb,weights);color=rgb+vec3<f32>(clamp(lookup(gray,0u),0.0,1.0)-gray);
    let lum=dot(color,weights);let low=min(color.r,min(color.g,color.b));let high=max(color.r,max(color.g,color.b));
    if low<0.0 {color=vec3<f32>(lum)+(color-vec3<f32>(lum))*(lum/(lum-low));}
    else if high>1.0 {color=vec3<f32>(lum)+(color-vec3<f32>(lum))*((1.0-lum)/(high-lum));}
 }
 let q=vec3<u32>(round(clamp(color,vec3<f32>(0.0),vec3<f32>(1.0))*255.0));output[i]=q.r|(q.g<<8u)|(q.b<<16u)|(p&0xff000000u);
}
