struct Params { width: u32, height: u32, layers: u32, unused: u32 }
struct Layer { opacity: u32, mode: u32, visible: u32, unused: u32 }
@group(0) @binding(0) var<storage, read> pixels: array<u32>;
@group(0) @binding(1) var<storage, read> layers: array<Layer>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
@group(0) @binding(4) var display: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(5) var<storage, read> selection: array<u32>;

fn selected(at: vec2<i32>) -> bool {
    if any(at < vec2(0)) || at.x >= i32(params.width) || at.y >= i32(params.height) { return false; }
    let i = u32(at.y) * params.width + u32(at.x);
    return (selection[i / 32u] & (1u << (i % 32u))) != 0u;
}

fn scale(a: u32, b: u32) -> u32 { let t = a*b+128u; return (t+(t>>8u))>>8u; }
fn component(d: u32, s: u32, mode: u32) -> u32 {
    switch mode {
        case 0u: { return s; }
        case 1u: { return scale(d,s); }
        case 2u: { return min(255u,d+s); }
        case 3u: { if s==0u { return 0u; } return u32(max(0,255-i32((255u-d)*255u/s))); }
        case 4u: { if s==255u { return 255u; } return min(255u,d*255u/(255u-s)); }
        case 5u: { if s==255u { return 255u; } return min(255u,d*d/(255u-s)); }
        case 6u: { if d==255u { return 255u; } return min(255u,s*s/(255u-d)); }
        case 7u: { if d<128u { return scale(d*2u,s); } return 255u-scale((255u-d)*2u,255u-s); }
        case 8u: { return u32(abs(i32(d)-i32(s))); }
        case 9u: { return 255u-u32(abs(255-i32(d)-i32(s))); }
        case 10u: { return max(d,s); }
        case 11u: { return min(d,s); }
        case 12u: { return 255u-scale(255u-d,255u-s); }
        default: { return d^s; }
    }
}
fn unpack(p: u32) -> vec4<u32> { return vec4<u32>(p&255u,(p>>8u)&255u,(p>>16u)&255u,p>>24u); }
fn pack(p: vec4<u32>) -> u32 { return p.r|(p.g<<8u)|(p.b<<16u)|(p.a<<24u); }
fn blend(dst: vec4<u32>, src: vec4<u32>, opacity: u32, mode: u32) -> vec4<u32> {
    let sa = scale(src.a,opacity);
    if sa==0u { return dst; }
    let remaining = scale(dst.a,255u-sa);
    let alpha = remaining+sa;
    let overlap = scale(dst.a,sa);
    var out: vec4<u32>;
    out.a = alpha;
    for (var c=0u; c<3u; c++) {
        out[c] = (dst[c]*remaining + src[c]*(sa-overlap) + component(dst[c],src[c],mode)*overlap)/alpha;
    }
    return out;
}
@compute @workgroup_size(16,16)
fn main(@builtin(global_invocation_id) pos: vec3<u32>) {
    if pos.x>=params.width || pos.y>=params.height { return; }
    let count = params.width*params.height;
    let i = pos.y*params.width+pos.x;
    var color = vec4<u32>(0u);
    for (var l=0u; l<params.layers; l++) {
        if layers[l].visible!=0u {
            color = blend(color,unpack(pixels[l*count+i]),layers[l].opacity,layers[l].mode);
        }
    }
    output[i] = pack(color);
    let checker = select(205u,238u,((pos.x/8u+pos.y/8u)%2u)==0u);
    var visible = blend(vec4<u32>(checker,checker,checker,255u),color,255u,0u);
    if params.unused != 0u {
        let at = vec2<i32>(pos.xy);
        if selected(at) && (!selected(at-vec2(1,0)) || !selected(at+vec2(1,0)) || !selected(at-vec2(0,1)) || !selected(at+vec2(0,1))) {
            let stripe = select(0u,255u,((pos.x+pos.y)/4u)%2u==0u);
            visible=vec4(stripe,stripe,stripe,255u);
        }
    }
    textureStore(display,vec2<i32>(pos.xy),vec4<f32>(visible)/255.0);
}
