struct Params { sw:u32, sh:u32, dw:u32, dh:u32, axis:u32, final_pass:u32, nearest:u32, linear_light:u32 }
struct Tap { index:u32, weight:f32 }
@group(0) @binding(0) var<storage,read> source:array<u32>;
@group(0) @binding(1) var<storage,read_write> intermediate:array<vec4<f32>>;
@group(0) @binding(2) var<storage,read_write> output:array<u32>;
@group(0) @binding(3) var<storage,read> spans:array<vec2<u32>>;
@group(0) @binding(4) var<storage,read> taps:array<Tap>;
@group(0) @binding(5) var<uniform> p:Params;
fn linear(v:vec3<f32>)->vec3<f32> { return select(pow((v+0.055)/1.055,vec3(2.4)),v/12.92,v<=vec3(0.04045)); }
fn srgb(v:vec3<f32>)->vec3<f32> { return select(1.055*pow(max(v,vec3(0.)),vec3(1./2.4))-0.055,12.92*v,v<=vec3(0.0031308)); }
fn decode(value:u32)->vec4<f32> {
    let v=vec4<f32>(f32(value&255u),f32((value>>8u)&255u),f32((value>>16u)&255u),f32(value>>24u))/255.;
    if p.nearest!=0u { return v; }
    var rgb=v.rgb;
    if p.linear_light!=0u { rgb=linear(rgb); }
    return vec4(rgb*v.a,v.a);
}
fn encode(value:vec4<f32>)->u32 {
    var color=value;
    if p.nearest==0u {
        var rgb=vec3(0.);
        if value.a>0.000001 { rgb=value.rgb/value.a; }
        if p.linear_light!=0u { rgb=srgb(rgb); }
        color=vec4(rgb,value.a);
    }
    let v=vec4<u32>(round(clamp(color,vec4(0.),vec4(1.))*255.));
    return v.r|(v.g<<8u)|(v.b<<16u)|(v.a<<24u);
}
@compute @workgroup_size(16,16)
fn main(@builtin(global_invocation_id) at:vec3<u32>) {
    if at.x>=p.dw || at.y>=p.dh { return; }
    let coordinate=select(at.x,at.y,p.axis==1u);
    let span=spans[coordinate];
    var sum=vec4(0.);
    for(var n=0u;n<span.y;n++) {
        let tap=taps[span.x+n];
        var xy=at.xy;
        if p.axis==0u { xy.x=tap.index; } else { xy.y=tap.index; }
        let i=xy.y*p.sw+xy.x;
        var value:vec4<f32>;
        if p.final_pass==0u { value=decode(source[i]); } else { value=intermediate[i]; }
        sum+=value*tap.weight;
    }
    let dst=at.y*p.dw+at.x;
    if p.final_pass==0u { intermediate[dst]=sum; } else { output[dst]=encode(sum); }
}
