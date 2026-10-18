struct Params {
    dims:vec4<u32>, origins:vec4<i32>, stage:vec4<u32>,
    geometry:vec4<f32>, reconstruction:vec4<f32>,
}
@group(0) @binding(0) var<storage,read> packed:array<u32>;
@group(0) @binding(1) var<storage,read> selection:array<u32>;
@group(0) @binding(2) var<storage,read> source:array<vec4<f32>>;
@group(0) @binding(3) var<storage,read_write> destination:array<vec4<f32>>;
@group(0) @binding(4) var<storage,read_write> output:array<u32>;
@group(0) @binding(5) var<storage,read> taps:array<vec4<f32>>;
@group(0) @binding(6) var<uniform> p:Params;
fn linear(v:vec3<f32>)->vec3<f32>{return select(pow((v+vec3(0.055))/1.055,vec3(2.4)),v/12.92,v<=vec3(0.04045));}
fn srgb(v:vec3<f32>)->vec3<f32>{return select(1.055*pow(max(v,vec3(0.)),vec3(1./2.4))-vec3(0.055),v*12.92,v<=vec3(0.0031308));}
fn mirror(n:i32,size:i32)->i32{let q=((n%(size*2))+size*2)%(size*2);return select(q,size*2-1-q,q>=size);}
fn original(q:vec2<i32>)->vec4<f32>{
    let at=vec2<u32>(vec2<i32>(mirror(q.x,i32(p.dims.x)),mirror(q.y,i32(p.dims.y))));
    let word=packed[at.y*p.dims.x+at.x];
    let color=vec4<f32>(f32(word&255u),f32((word>>8u)&255u),f32((word>>16u)&255u),f32(word>>24u))/255.;
    let rgb=pow(max(linear(color.rgb),vec3(0.)),vec3(p.geometry.z));
    return vec4(rgb*color.a,color.a);
}
fn original_sample(point:vec2<f32>)->vec4<f32>{
    let at=point-vec2(0.5);let q=vec2<i32>(floor(at));let t=fract(at);
    return mix(mix(original(q),original(q+vec2(1,0)),t.x),mix(original(q+vec2(0,1)),original(q+vec2(1)),t.x),t.y);
}
fn cached(q:vec2<i32>)->vec4<f32>{let at=clamp(q,vec2<i32>(0),vec2<i32>(p.stage.zw)-vec2<i32>(1));return source[u32(at.y)*p.stage.z+u32(at.x)];}
fn sample_linear(point:vec2<f32>)->vec4<f32>{
    let at=point-vec2<f32>(p.origins.xy)-vec2(0.5);let q=vec2<i32>(floor(at));let t=fract(at);
    return mix(mix(cached(q),cached(q+vec2(1,0)),t.x),mix(cached(q+vec2(0,1)),cached(q+vec2(1)),t.x),t.y);
}
fn cubic_weight(value:f32)->f32{let x=abs(value);if x<1.{return (1.5*x-2.5)*x*x+1.;}if x<2.{return ((-0.5*x+2.5)*x-4.)*x+2.;}return 0.;}
fn sample_cubic(point:vec2<f32>)->vec4<f32>{
    let at=point-vec2<f32>(p.origins.xy)-vec2(0.5);let q=vec2<i32>(floor(at));let t=fract(at);var sum=vec4(0.);
    for(var y=-1;y<=2;y++){for(var x=-1;x<=2;x++){sum+=cached(q+vec2(x,y))*cubic_weight(f32(x)-t.x)*cubic_weight(f32(y)-t.y);}}return sum;
}
@compute @workgroup_size(16,16)
fn main(@builtin(global_invocation_id) at:vec3<u32>){
    if any(at.xy>=p.dims.zw){return;}
    let i=at.y*p.dims.z+at.x;
    let q=vec2<f32>(vec2<i32>(at.xy)+p.origins.zw);
    if p.stage.x==0u{destination[i]=original_sample((q+vec2(0.5))/p.geometry.x);return;}
    if p.stage.x==1u || p.stage.x==2u{
        let axis=select(vec2(1.,0.),vec2(0.,1.),p.stage.x==2u);
        let center=q*(vec2(1.)+axis*(1./p.geometry.y-1.))+vec2(0.5);
        var sum=vec4(0.);
        for(var n=0u;n<arrayLength(&taps);n++){
            let tap=taps[n];sum+=(sample_linear(center+axis*tap.x)+sample_linear(center-axis*tap.x))*tap.z;
        }
        destination[i]=sum;return;
    }
    if selection[i]==0u{output[i]=packed[i];return;}
    let point=(q+vec2(0.5)-vec2(p.reconstruction.y))/p.reconstruction.x;
    var value=sample_linear(point);
    if p.stage.y!=0u{value=sample_cubic(point);}
    var rgb=vec3(0.);if value.a>0.000001{rgb=value.rgb/value.a;}
    rgb=srgb(pow(max(rgb,vec3(0.)),vec3(1./p.geometry.z)));
    let color=vec4<u32>(round(clamp(vec4(rgb,value.a),vec4(0.),vec4(1.))*255.));
    output[i]=color.r|(color.g<<8u)|(color.b<<16u)|(color.a<<24u);
}
