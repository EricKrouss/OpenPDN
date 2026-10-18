struct Params { width: u32, height: u32, operation: u32, unused: u32, values: vec4<f32>, bounds: vec4<u32> }
@group(0) @binding(0) var<storage, read> pixels: array<u32>;
@group(0) @binding(1) var<storage, read> selection: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
// PosterizeEffect2.GenerateLevels: the counter advances only after exceeding 255.
fn posterize(value: u32, count: u32) -> u32 {
    if count==0u { return value; }
    let index = (max(value*count, 1u)-1u)/255u;
    return 255u*index/(count-1u);
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
fn srgb_to_linear(v: vec3<f32>) -> vec3<f32> {
    return select(pow((v+vec3<f32>(0.055))/1.055,vec3<f32>(2.4)),v/12.92,v<=vec3<f32>(0.04045));
}
fn linear_to_srgb(v: vec3<f32>) -> vec3<f32> {
    return select(1.055*pow(max(v,vec3<f32>(0.0)),vec3<f32>(1.0/2.4))-vec3<f32>(0.055),v*12.92,v<=vec3<f32>(0.0031308));
}
fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let hi=max(c.r,max(c.g,c.b)); let lo=min(c.r,min(c.g,c.b)); let d=hi-lo; let l=(hi+lo)*0.5;
    if d==0.0 { return vec3<f32>(0.0,0.0,l); }
    var h=(c.g-c.b)/d;
    if hi==c.g { h=(c.b-c.r)/d+2.0; } else if hi==c.b { h=(c.r-c.g)/d+4.0; }
    return vec3<f32>(fract(h/6.0),d/(1.0-abs(2.0*l-1.0)),l);
}
fn hsl_to_rgb(v: vec3<f32>) -> vec3<f32> {
    let chroma=(1.0-abs(2.0*v.z-1.0))*v.y;
    let h=fract(v.x)*6.0; let x=chroma*(1.0-abs((h-2.0*floor(h/2.0))-1.0));
    var c=vec3<f32>(chroma,x,0.0);
    if h>=5.0 { c=vec3<f32>(chroma,0.0,x); }
    else if h>=4.0 { c=vec3<f32>(x,0.0,chroma); }
    else if h>=3.0 { c=vec3<f32>(0.0,x,chroma); }
    else if h>=2.0 { c=vec3<f32>(0.0,chroma,x); }
    else if h>=1.0 { c=vec3<f32>(x,chroma,0.0); }
    return c+vec3<f32>(v.z-chroma*0.5);
}
fn gradient_lerp(point: vec2<f32>) -> f32 {
    let start=params.values.xy; let end=params.values.zw; let delta=end-start;
    let p=point-start; let len=length(delta); let shape=params.unused&7u; let repeat=(params.unused>>3u)&3u;
    if len==0.0 { return 1.0; }
    let step=bitcast<vec2<f32>>(params.bounds.zw); let projection=p.x*step.x+p.y*step.y; let across=p.x*step.y-p.y*step.x;
    // HLSL/Metal do not promise atan2(0,0) == 0. The managed reference does.
    // Guard the input too: native fast-math compilers may speculate the
    // intrinsic across an output branch and propagate its undefined result.
    let angle_point=select(p,vec2<f32>(1.0,0.0),all(p==vec2<f32>(0.0)));
    let point_angle=atan2(angle_point.y,angle_point.x);
    var t=projection;
    if shape==1u { t=abs(t); } else if shape==2u { t=abs(t)+abs(across); }
    else if shape==3u { t=length(p)/len; }
    else if shape==4u {
        if repeat==0u { t=(point_angle-atan2(delta.y,delta.x))/6.28318530718; if t<0.0 { t+=1.0; } }
        else { t=(point_angle-atan2(-delta.y,-delta.x))/3.14159265359; }
    } else if shape>=5u {
        var angle=point_angle-atan2(delta.y,delta.x);
        if shape==5u { angle=-angle; }
        if angle<0.0 { angle+=6.28318530718; }
        t=angle/6.28318530718;
        if repeat==2u { t=t*2.0+length(p)/len; } else { t+=length(p)/len; }
    }
    if repeat==1u {
        // Preserve the original negative-integer endpoint behavior.
        if t<0.0 { return 1.0+(t-f32(i32(t))); } return t-f32(i32(t));
    }
    if repeat==2u { t=t-2.0*floor(t/2.0); if t>1.0 {t=2.0-t;} return t; }
    return clamp(t,0.0,1.0);
}
fn overwrite(dst:vec4<u32>, src:vec4<u32>, coverage:u32)->vec4<u32> {
    if coverage==0u { return dst; } if coverage==255u { return src; }
    let da=scale(255u-coverage,dst.a); let sa=scale(coverage,src.a); let alpha=da+sa;
    if alpha==0u { return vec4<u32>(0u); }
    return vec4<u32>((dst.rgb*da+src.rgb*sa)/alpha,alpha);
}
fn gradient(dst:vec4<u32>,point:vec2<f32>,coverage:u32)->vec4<u32> {
    let t=gradient_lerp(point); let a=unpack(params.bounds.x); let b=unpack(params.bounds.y);
    if (params.unused&32u)!=0u {
        var alpha=u32(clamp(f32(a.a)+(f32(b.a)-f32(a.a))*t+0.5,0.0,255.0));
        if (params.unused&64u)==0u { alpha=scale(dst.a,alpha); }
        return overwrite(dst,vec4<u32>(dst.rgb,alpha),coverage);
    }
    let weight=u32(t*255.0); let wa=scale(255u-weight,a.a); let wb=scale(weight,b.a); let alpha=wa+wb;
    var color=vec4<u32>(0u);
    if alpha!=0u { color=vec4<u32>((a.rgb*wa+b.rgb*wb)/alpha,alpha); }
    if all(params.values.xy==params.values.zw) || all(a==b) { color=b; }
    if (params.unused&64u)!=0u { return overwrite(dst,color,coverage); }
    return blend(dst,color,coverage,(params.unused>>7u)&15u);
}
fn mirror(n:i32,size:i32)->i32 { let period=size*2; var p=((n%period)+period)%period; if p>=size {p=period-1-p;} return p; }
fn relief(i:u32)->vec4<f32> {
    let x=i32(i%params.width); let y=i32(i/params.width);
    let angle=params.values.x*0.0174532925199433; let q=0.7853981633974483;
    let kernel=array<f32,9>(cos(angle-3.0*q),cos(angle-2.0*q),cos(angle-q),cos(angle+4.0*q),1.0,cos(angle),cos(angle+3.0*q),cos(angle+2.0*q),cos(angle+q));
    var result=vec4<f32>(0.0);
    for(var j=0u;j<9u;j++) {
        let xx=mirror(x+i32(j%3u)-1,i32(params.width)); let yy=mirror(y+i32(j/3u)-1,i32(params.height));
        result+=vec4<f32>(unpack(pixels[u32(yy)*params.width+u32(xx)]))/255.0*kernel[j];
    }
    return result;
}
fn emboss(i:u32)->vec4<u32> {
    let x=i32(i%params.width);let y=i32(i/params.width);
    let angle=(params.values.x-180.0)*0.0174532925199433;let q=0.7853981633974483;
    let kernel=array<f32,9>(cos(angle+q),cos(angle+2.0*q),cos(angle+3.0*q),cos(angle),0.0,cos(angle+4.0*q),cos(angle-q),cos(angle-2.0*q),cos(angle-3.0*q));
    var gray=128.0/255.0;
    for(var j=0u;j<9u;j++) {let xx=mirror(x+i32(j%3u)-1,i32(params.width));let yy=mirror(y+i32(j/3u)-1,i32(params.height));let rgb=vec3<f32>(unpack(pixels[u32(yy)*params.width+u32(xx)]).rgb)/255.0;gray+=dot(rgb,vec3<f32>(0.299,0.587,0.114))*kernel[j];}
    return vec4<u32>(vec3<u32>(u32(round(clamp(gray,0.0,1.0)*255.0))),unpack(pixels[i]).a);
}
fn morphology(i:u32)->vec4<u32> {
    let x=i32(i%params.width);let y=i32(i/params.width);let w=i32(params.values.x);let h=i32(params.values.y);let erode=params.values.z!=0.0;
    var result=vec4<f32>(select(0.0,1.0,erode));
    for(var yy=0;yy<h;yy++) {for(var xx=0;xx<w;xx++) {
        let sx=x+xx-w/2;let sy=y+yy-h/2;var p=vec4<f32>(0.0);
        if sx>=0 && sy>=0 && sx<i32(params.width) && sy<i32(params.height) {let c=vec4<f32>(unpack(pixels[u32(sy)*params.width+u32(sx)]))/255.0;p=vec4<f32>(c.rgb*c.a,c.a);}
        if erode {result=min(result,p);}else {result=max(result,p);}
    }}
    if result.a>0.0 {result=vec4<f32>(result.rgb/result.a,result.a);}
    return vec4<u32>(round(clamp(result,vec4<f32>(0.0),vec4<f32>(1.0))*255.0));
}
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) pos: vec3<u32>) {
    let i = pos.x;
    if i>=params.width*params.height { return; }
    let p = pixels[i];
    if selection[i]==0u { output[i]=p; return; }
    if params.operation==10u { output[i]=pack(gradient(unpack(p),vec2<f32>(f32(i%params.width),f32(i/params.width)),selection[i])); }
    else if params.operation==11u {output[i]=pack(emboss(i));}
    else if params.operation==12u {output[i]=pack(morphology(i));}
    else if params.operation==9u { output[i]=pack(vec4<u32>(round(clamp(relief(i),vec4<f32>(0.0),vec4<f32>(1.0))*255.0))); }
    else if params.operation==0u { output[i]=(p^0x00ffffffu); }
    else if params.operation==1u { output[i]=(p^0xff000000u); }
    else if params.operation==4u {
        let c=vec4<u32>(p&255u,(p>>8u)&255u,(p>>16u)&255u,p>>24u);
        let counts=vec4<u32>(params.values);
        let v=vec4<u32>(posterize(c.r,counts.r),posterize(c.g,counts.g),posterize(c.b,counts.b),posterize(c.a,counts.a));
        output[i]=v.r|(v.g<<8u)|(v.b<<16u)|(v.a<<24u);
    } else {
        // GrayscaleEffect2's LumaShader, straight alpha and sRGB input gamma.
        let rgb=vec3<f32>(f32(p&255u),f32((p>>8u)&255u),f32((p>>16u)&255u))/255.0;
        let gray=dot(rgb,vec3<f32>(0.299,0.587,0.114));
        var color=vec3<f32>(gray);
        if params.operation==3u {
            let intensity=params.values.x/100.0;
            color=vec3<f32>(pow(gray,1.0-0.4*intensity),gray,pow(gray,1.0+0.4*intensity));
        }
        if params.operation==5u {
            let brightness=params.values.x/255.0; let contrast=params.values.y;
            if contrast==100.0 { color=vec3<f32>(select(0.0,1.0,gray+brightness>=0.5)); }
            else {
                var multiply=1.0; var divisor=1.0;
                if contrast<0.0 { multiply=contrast+100.0; divisor=100.0; }
                else if contrast>0.0 { multiply=100.0; divisor=100.0-contrast; }
                var pre=brightness; var post=0.0;
                if divisor==100.0 { pre=0.0; post=brightness; }
                let shift=(gray-0.5+pre)*multiply/divisor+(0.5-gray+post);
                color=rgb+vec3<f32>(shift);
            }
        } else if params.operation==16u {
            color=rgb*params.values.rgb;
        } else if params.operation==6u {
            color=linear_to_srgb(srgb_to_linear(rgb)*exp2(params.values.x/100.0));
        } else if params.operation==7u {
            var hsl=rgb_to_hsl(rgb); hsl.x+=params.values.x/360.0;
            var saturation=params.values.y/100.0;
            if saturation>1.0 { saturation=(saturation-1.0)*3.0+1.0; }
            hsl.y=clamp(hsl.y*saturation,0.0,1.0);
            color=hsl_to_rgb(hsl);
            color=mix(color,vec3<f32>(select(0.0,1.0,params.values.z>=0.0)),abs(params.values.z)/100.0);
            if (p>>24u)==0u { color=vec3<f32>(0.0); }
        } else if params.operation==8u {
            let bounds=bitcast<vec4<f32>>(params.bounds);
            let center=bounds.xy+bounds.zw*(vec2<f32>(1.0)+params.values.xy)*0.5;
            let position=vec2<f32>(f32(i%params.width)+0.5,f32(i/params.width)+0.5)-center;
            let radius=max(bounds.z,bounds.w)*0.5*params.values.z;
            let d=dot(position,position)*3.14159265359/(8.0*radius*radius);
            let cs=cos(d); var factor=1.0-params.values.w;
            if cs>0.0 && d<3.14159265359 { factor+=params.values.w*cs*cs*cs*cs; }
            color=linear_to_srgb(srgb_to_linear(rgb)*factor);
        }
        let out=vec3<u32>(round(clamp(color,vec3<f32>(0.0),vec3<f32>(1.0))*255.0));
        output[i]=out.r|(out.g<<8u)|(out.b<<16u)|(p&0xff000000u);
    }
}
