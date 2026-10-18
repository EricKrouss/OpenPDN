override operation:u32=0u;
// Float RGBA intermediates. unused.x selects straight output for filters
// whose original shader processes RGBA independently (including hidden RGB).
struct Params { width:u32, height:u32, operation:u32, axis:u32, a:vec4<f32>, b:vec4<f32>, linear:u32, gamma:f32, unused:vec2<u32>, bounds:vec4<f32> }
@group(0) @binding(0) var<storage,read> packed:array<u32>;
@group(0) @binding(1) var<storage,read> selection:array<u32>;
@group(0) @binding(2) var<storage,read> source:array<vec4<f32>>;
@group(0) @binding(3) var<storage,read_write> destination:array<vec4<f32>>;
@group(0) @binding(4) var<storage,read_write> output:array<u32>;
@group(0) @binding(5) var<uniform> p:Params;
@group(0) @binding(6) var<storage,read> kernel:array<vec4<f32>>;
@group(0) @binding(7) var<storage,read_write> auxiliary:array<vec4<f32>>;
fn raw(i:u32)->vec4<f32>{let c=packed[i];return vec4<f32>(f32(c&255u),f32((c>>8u)&255u),f32((c>>16u)&255u),f32(c>>24u))/255.;}
fn linearize(v:vec3<f32>)->vec3<f32>{return select(pow((v+vec3<f32>(0.055))/1.055,vec3<f32>(2.4)),v/12.92,v<=vec3<f32>(0.04045));}
fn srgb(v:vec3<f32>)->vec3<f32>{return select(1.055*pow(max(v,vec3<f32>(0.)),vec3<f32>(1./2.4))-vec3<f32>(0.055),v*12.92,v<=vec3<f32>(0.0031308));}
fn mirror(x:i32,size:i32)->i32{let r=((x%(size*2))+size*2)%(size*2);return select(r,size*2-1-r,r>=size);}
// mode 0 transparent, 1 clamp, 2 mirror. The coverage channel is separate
// from alpha: transparent source pixels still count as valid input samples.
fn tap(q:vec2<i32>,mode:u32)->vec4<f32>{
    var at=q;let size=vec2<i32>(i32(p.width),i32(p.height));
    if mode==0u && (any(q<vec2<i32>(0)) || any(q>=size)){return vec4<f32>(0.);}
    if mode==1u{at=clamp(q,vec2<i32>(0),size-vec2<i32>(1));}
    if mode==2u{at=vec2<i32>(mirror(q.x,size.x),mirror(q.y,size.y));}
    if mode==3u{at=((q%size)+size)%size;}
    return source[u32(at.y)*p.width+u32(at.x)];
}
fn coverage(q:vec2<i32>)->f32{return select(0.,1.,all(q>=vec2<i32>(0)) && all(q<vec2<i32>(i32(p.width),i32(p.height))));}
fn sample_at(q:vec2<f32>,mode:u32)->vec4<f32>{let at=q-vec2<f32>(0.5);let lo=vec2<i32>(floor(at));let t=fract(at);return mix(mix(tap(lo,mode),tap(lo+vec2<i32>(1,0),mode),t.x),mix(tap(lo+vec2<i32>(0,1),mode),tap(lo+vec2<i32>(1,1),mode),t.x),t.y);}
fn sample_coverage(q:vec2<f32>)->f32{let at=q-vec2<f32>(0.5);let lo=vec2<i32>(floor(at));let t=fract(at);return mix(mix(coverage(lo),coverage(lo+vec2<i32>(1,0)),t.x),mix(coverage(lo+vec2<i32>(0,1)),coverage(lo+vec2<i32>(1,1)),t.x),t.y);}
fn luma(v:vec3<f32>)->f32{return dot(v,vec3<f32>(0.21266,0.71515,0.07219));}
fn original(q:vec2<i32>)->vec4<f32>{let at=clamp(q,vec2<i32>(0),vec2<i32>(i32(p.width)-1,i32(p.height)-1));let v=raw(u32(at.y)*p.width+u32(at.x));return vec4<f32>(v.rgb*v.a,v.a);}
fn straight(c:vec4<f32>)->vec3<f32>{if c.a>0.{return c.rgb/c.a;}return c.rgb;}
fn brightness_contrast(rgb:vec3<f32>,brightness:f32,contrast:f32)->vec3<f32>{
    let y=dot(rgb,vec3<f32>(0.299,0.587,0.114));let b=brightness/255.;
    if contrast>=100.{return vec3<f32>(select(0.,1.,y+b>=0.5));}
    var mul=1.;var div=1.;if contrast<0.{mul=contrast+100.;div=100.;}else if contrast>0.{mul=100.;div=100.-contrast;}
    var pre=b;var post=0.;if div==100.{pre=0.;post=b;}
    return clamp(rgb+vec3<f32>((y-0.5+pre)*mul/div+0.5-y+post),vec3<f32>(0.),vec3<f32>(1.));
}
fn blend_premultiplied(dst:vec4<f32>,src:vec4<f32>,rgb:vec3<f32>)->vec4<f32>{return vec4<f32>(dst.rgb*(1.-src.a)+src.rgb*(1.-dst.a)+rgb*dst.a*src.a,dst.a+src.a-dst.a*src.a);}
fn raw_mirror(q:vec2<i32>)->vec4<f32>{return raw(u32(mirror(q.y,i32(p.height)))*p.width+u32(mirror(q.x,i32(p.width))));}
fn raw_clamp(q:vec2<i32>)->vec4<f32>{let at=clamp(q,vec2<i32>(0),vec2<i32>(i32(p.width)-1,i32(p.height)-1));return raw(u32(at.y)*p.width+u32(at.x));}
// HLSL f32tof16 truncates positive values. WGSL packs to nearest: adjust
// that result to recover the original half-float floor and ceiling bounds.
fn half_bound(v:f32,ceiling:bool)->f32{
    var bits=pack2x16float(vec2<f32>(v,0.))&65535u;
    if unpack2x16float(bits).x>v{bits-=1u;}
    if ceiling && unpack2x16float(bits).x<v{bits+=1u;}
    return unpack2x16float(bits).x;
}
fn percentile_at(q:vec2<i32>,radius:i32,percentile:f32,quality:u32)->vec4<f32>{
    let squared=((2*radius+1)*(2*radius+1)+2)/4;
    var lo=vec4<f32>(1.);var hi=vec4<f32>(0.);var area=0.;
    let endpoint=percentile<=0. || percentile>=1.;
    for(var dy=-radius;dy<=radius;dy++){for(var dx=-radius;dx<=radius;dx++){
        let circle=dx*dx+dy*dy<=squared;
        if circle{area+=1.;}
        if circle || (!endpoint && quality>6u){let c=raw_clamp(q+vec2<i32>(dx,dy));lo=min(lo,c);hi=max(hi,c);}
    }}
    if percentile<=0.{return lo;}if percentile>=1.{return hi;}
    for(var c=0u;c<4u;c++){lo[c]=half_bound(lo[c],false);hi[c]=half_bound(hi[c],true);}
    let target_area=area*percentile;var pivot=(lo+hi)*0.5;
    // The native even-quality four-way search is two binary steps per pass.
    // Use the same quarter boundaries to retain its rounding and tie rules.
    let steps=select(1u,2u,(quality&1u)==0u);
    for(var iteration=0u;iteration<quality;iteration+=steps){
        let m1=select(pivot,(lo+pivot)*0.5,steps==2u);let m3=(pivot+hi)*0.5;
        var a1=vec4<f32>(0.);var a2=vec4<f32>(0.);var a3=vec4<f32>(0.);
        for(var dy=-radius;dy<=radius;dy++){for(var dx=-radius;dx<=radius;dx++){if dx*dx+dy*dy<=squared{
            let s=raw_clamp(q+vec2<i32>(dx,dy));a1+=select(vec4<f32>(0.),vec4<f32>(1.),s<=m1);
            if steps==2u{a2+=select(vec4<f32>(0.),vec4<f32>(1.),s<=pivot);a3+=select(vec4<f32>(0.),vec4<f32>(1.),s<=m3);}
        }}}
        if steps==1u{let lower=vec4<f32>(target_area)<=a1;lo=select(pivot,lo,lower);hi=select(hi,pivot,lower);}
        else{let b1=vec4<f32>(target_area)<=a1;let b2=vec4<f32>(target_area)<=a2;let b3=vec4<f32>(target_area)<=a3;lo=select(select(select(m3,pivot,b3),m1,b2),lo,b1);hi=select(select(select(hi,m3,b3),pivot,b2),m1,b1);}
        pivot=(lo+hi)*0.5;
    }
    return pivot;
}
fn oil_sample(q:vec2<i32>)->vec4<f32>{let c=raw_clamp(q);return vec4<f32>(linearize(c.rgb),c.a);}
fn oil_bin(c:vec4<f32>,bins:u32)->u32{return u32(clamp(dot(c.rgb,vec3<f32>(0.299,0.587,0.114)),0.,1.)*f32(bins-1u));}
fn pcg(input:u32)->u32{let state=input*747796405u+2891336453u;let word=((state>>((state>>28u)+4u))^state)*277803737u;return (word>>22u)^word;}
fn hash_combine(a:u32,b:u32)->u32{return ((a<<5u)+a)^b;}
fn pixel_seed(instance:u32,at:vec2<f32>)->u32{return hash_combine(instance,hash_combine(pcg(bitcast<u32>(at.x)),pcg(bitcast<u32>(at.y))));}
fn next_float(seed:ptr<function,u32>)->f32{*seed=pcg(*seed);return f32(*seed>>8u)*5.9604645e-08;}
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid:vec3<u32>){
    if operation==42u{var index=gid.x;var q=vec2<i32>(i32(index%p.width),i32(index/p.width));if p.a.z!=0.{q=vec2<i32>(i32(p.axis)-2*i32(index),i32(index));if q.x<0 || q.x>=i32(p.width) || q.y>=i32(p.height){return;}index=u32(q.y)*p.width+u32(q.x);}if index<p.width*p.height{quantize_pixel(q,index);}return;}
    let i=gid.x;if operation==60u{let size=vec2<u32>(kernel[0].zw);if i<size.x*size.y{let at=vec2<f32>(f32(i%size.x),f32(i/size.x))+kernel[0].xy+vec2<f32>(0.5);destination[i]=pixel_down(at);}return;}if i>=p.width*p.height{return;}
    let q=vec2<i32>(i32(i%p.width),i32(i/p.width));let point=vec2<f32>(q)+vec2<f32>(0.5);
    if operation==0u {var c=raw(i);if p.linear!=0u{c=vec4<f32>(pow(linearize(c.rgb),vec3<f32>(p.gamma)),c.a);}destination[i]=vec4<f32>(c.rgb*c.a,c.a);return;}
    if operation==1u {
        if selection[i]==0u && p.unused.y==0u{output[i]=packed[i];return;}
        let c=source[i];var rgb=vec3<f32>(0.);if c.a>0.{rgb=c.rgb/c.a;}
        if p.unused.x!=0u{rgb=c.rgb;}
        if p.linear!=0u{rgb=srgb(pow(max(rgb,vec3<f32>(0.)),vec3<f32>(1./p.gamma)));}
        let b=vec4<u32>(round(clamp(vec4<f32>(rgb,c.a),vec4<f32>(0.),vec4<f32>(1.))*255.));output[i]=b.r|(b.g<<8u)|(b.b<<16u)|(b.a<<24u);return;
    }
    if operation==6u || operation==7u{
        var sum=vec4<f32>(0.);let size=u32(p.b.x);let base=u32(p.a.w);for(var j=0u;j<size;j++){let k=kernel[base+j];sum+=tap(q+vec2<i32>(0,i32(j)-i32(size/2u)),2u)*select(k.x,k.y,operation==7u);}destination[i]=sum;return;
    }
    if operation==8u{
        var sum=vec4<f32>(0.);let size=u32(p.b.x);let base=u32(p.a.w);for(var j=0u;j<size;j++){
            let at=vec2<i32>(mirror(q.x+i32(j)-i32(size/2u),i32(p.width)),q.y);let index=u32(at.y)*p.width+u32(at.x);let re=source[index];let im=auxiliary[index];let k=kernel[base+j];sum+=k.z*(re*k.x-im*k.y)+k.w*(re*k.y+im*k.x);
        }if p.axis==0u{destination[i]=sum;}else{destination[i]+=sum;}return;
    }
    if operation==41u{
        let iterations=u32(p.a.z);let count=arrayLength(&kernel)/iterations;var sum=vec4<f32>(0.);for(var iteration=0u;iteration<iterations;iteration++){var color=vec4<f32>(0.);for(var channel=0u;channel<4u;channel++){color[channel]=sketch_quantile(point,iteration*count,count,channel);}sum+=vec4<f32>(linearize(color.rgb),color.a);}sum/=f32(iterations);destination[i]=vec4<f32>(srgb(sum.rgb),sum.a);return;
    }
    if operation==2u {var sum=vec4<f32>(0.);for(var j=0u;j<arrayLength(&kernel);j++){let v=kernel[j];let offset=select(vec2<i32>(i32(v.x),0),vec2<i32>(0,i32(v.x)),p.axis!=0u);sum+=tap(q+offset,2u)*v.z;}destination[i]=sum;return;}
    if operation==5u {
        let angle=-p.a.z*0.01745329252;let shift=p.a.y*vec2<f32>(cos(angle),sin(angle));var sum=vec4<f32>(0.);
        for(var j=0u;j<arrayLength(&kernel);j++){let k=kernel[j];let offset=select(vec2<f32>(k.x-shift.x,0.),vec2<f32>(0.,k.x-shift.y),p.axis!=0u);sum+=sample_at(point+offset,0u)*k.z;}
        destination[i]=sum;return;
    }
    if operation==3u {
        let dy=luma(straight(tap(q+vec2<i32>(0,1),1u)))-luma(straight(tap(q-vec2<i32>(0,1),1u)));
        let dx=luma(straight(tap(q+vec2<i32>(1,0),1u)))-luma(straight(tap(q-vec2<i32>(1,0),1u)));
        destination[i]=vec4<f32>(straight(source[i]),(dx*dx+dy*dy)*p.a.w*p.a.w*0.5*3.141592741*0.7);return;
    }
    if operation==4u {
        let c=raw(i);let mask=source[i];let blurred=mask.rgb;
        let rgb=pow(max(c.rgb,vec3<f32>(0.)),vec3<f32>(1./2.2));
        let y=luma(rgb);let blur_y=luma(pow(max(blurred,vec3<f32>(0.)),vec3<f32>(1./2.2)));
        let local=mix(blur_y,y,mask.a);let detail=y+p.a.z*(y-blur_y);
        let highlight=y*p.a.x/(1.+y*(p.a.x-1.));let shadow=y*p.a.y/(1.+y*(p.a.y-1.));
        let hi=local*local;let lo=(1.-local)*(1.-local);let adjusted=clamp(hi*highlight+lo*shadow+(1.-hi-lo)*detail,0.,1.);
        let u=dot(rgb,vec3<f32>(-0.09991,-0.33609,0.436));let v=dot(rgb,vec3<f32>(0.615,-0.55861,-0.05639));
        var result=vec3<f32>(adjusted+1.28033*v,adjusted-0.21482*u-0.38059*v,adjusted+2.12798*u);
        result/=max(1.,max(result.r,max(result.g,result.b)));
        destination[i]=vec4<f32>(pow(max(result,vec3<f32>(0.)),vec3<f32>(2.2))*c.a,c.a);return;
    }
    if operation==27u || operation==28u {
        let c=raw(i);let blurred=source[i];let rgb=straight(blurred);let alpha=c.a+blurred.a-c.a*blurred.a;
        var dst=c.rgb;var src=vec3<f32>(0.);var blended=vec3<f32>(0.);
        if operation==27u{src=brightness_contrast(rgb,p.a.y,p.a.z);blended=dst+src-dst*src;}
        else{let range=p.a.y+1.;src=vec3<f32>(dot(vec3<f32>(1.)-brightness_contrast(rgb,range,-range),vec3<f32>(0.299,0.587,0.114)));dst=vec3<f32>(dot(dst,vec3<f32>(0.299,0.587,0.114)));blended=min(vec3<f32>(1.),dst/max(vec3<f32>(0.000001),vec3<f32>(1.)-src));}
        destination[i]=vec4<f32>(dst*c.a*(1.-blurred.a)+src*blurred.a*(1.-c.a)+blended*c.a*blurred.a,alpha);return;
    }
    if operation==29u {
        var mode=1u;if p.a.w==1.{mode=3u;}else if p.a.w==2.{mode=2u;}else if p.a.w==3.{mode=0u;}
        var sum=vec4<f32>(0.);for(var j=0u;j<arrayLength(&kernel);j++){let k=kernel[j];sum+=tap(q+vec2<i32>(k.xy),mode)*k.z;}destination[i]=sum;return;
    }
    if operation==30u {
        let c=raw(i);let blurred=source[i];let coloring=100.-2.*p.a.y;
        let glow_rgb=brightness_contrast(straight(blurred),coloring,coloring);
        let glow=blend_premultiplied(vec4<f32>(c.rgb*c.a,c.a),vec4<f32>(glow_rgb*blurred.a,blurred.a),c.rgb+glow_rgb-c.rgb*glow_rgb);
        var convolved=vec3<f32>(0.);
        for(var dy=-2;dy<=2;dy++){for(var dx=-2;dx<=2;dx++){var w=-1.;if dx==0 && dy==0{w=30.;}else if dx==0 && dy==-2{w=-5.;}let t=raw_mirror(q+vec2<i32>(dx,dy));if t.a>0.{convolved+=t.rgb*w;}}}
        let gray=dot(clamp(convolved,vec3<f32>(0.),vec3<f32>(1.)),vec3<f32>(0.299,0.587,0.114));
        let ink=vec3<f32>(select(0.,1.,gray>p.a.x/100.));let outline=vec4<f32>(ink*c.a,c.a);
        destination[i]=blend_premultiplied(outline,glow,min(ink,straight(glow)));return;
    }
    if operation==31u {
        let center=raw(i);let radius=i32(p.a.x);let squared=((2*radius+1)*(2*radius+1)+2)/4;
        var sum=vec4<f32>(0.);var weight=vec4<f32>(0.);let slope=0.96/(p.a.y/100.);
        for(var dy=-radius;dy<=radius;dy++){for(var dx=-radius;dx<=radius;dx++){if dx*dx+dy*dy<=squared{let c=raw_mirror(q+vec2<i32>(dx,dy));let w=max(vec4<f32>(0.),vec4<f32>(1.)-abs(c-center)*slope);sum+=w*c;weight+=w;}}}
        let result=select(center,sum/max(weight,vec4<f32>(0.000001)),weight>vec4<f32>(0.));destination[i]=vec4<f32>(result.rgb*result.a,result.a);return;
    }
    if operation==32u {
        let c=raw(i);let blurred=source[i];let gray=dot(c.rgb,vec3<f32>(0.299,0.587,0.114));
        let dst=gray*vec3<f32>(1.+p.a.z/100.,1.,1.-p.a.z/100.);
        let src=brightness_contrast(straight(blurred),p.a.y,-p.a.y*0.5);
        let blended=select(2.*dst*src,vec3<f32>(1.)-2.*(vec3<f32>(1.)-dst)*(vec3<f32>(1.)-src),dst>=vec3<f32>(0.5));
        destination[i]=blend_premultiplied(vec4<f32>(dst*c.a,c.a),vec4<f32>(src*blurred.a,blurred.a),blended);return;
    }
    if operation==33u {
        let c=raw(i);let color=u32(p.b.x);let rgb=linearize(vec3<f32>(f32((color>>16u)&255u),f32((color>>8u)&255u),f32(color&255u))/255.);
        let alpha=source[i].a*p.a.w;let shadow=vec4<f32>(rgb*alpha,alpha);
        if p.b.y!=0.{destination[i]=shadow;}else{destination[i]=vec4<f32>(linearize(c.rgb)*c.a,c.a)+shadow*(1.-c.a);}return;
    }
    if operation==34u{destination[i]=percentile_at(q,i32(p.a.x),p.a.y/100.,u32(p.a.z));return;}
    if operation==35u{
        let radius=min(50,i32(p.a.x));let lo=percentile_at(q,radius,(1.-p.a.y/100.)*0.5,u32(p.a.z));let hi=percentile_at(q,radius,(1.+p.a.y/100.)*0.5,u32(p.a.z));destination[i]=vec4<f32>(vec3<f32>(1.)-(hi.rgb-lo.rgb),hi.a);return;
    }
    if operation==36u{
        let radius=i32(p.a.x);let squared=((2*radius+1)*(2*radius+1)+2)/4;let bins=clamp(u32(p.a.y*256.),4u,256u);
        var histogram:array<u32,256>;
        for(var dy=-radius;dy<=radius;dy++){for(var dx=-radius;dx<=radius;dx++){if p.a.z!=0. || dx*dx+dy*dy<=squared{histogram[oil_bin(oil_sample(q+vec2<i32>(dx,dy)),bins)]+=1u;}}}
        var mode=0u;for(var b=1u;b<bins;b++){if histogram[b]>histogram[mode]{mode=b;}}
        var sum=vec4<f32>(0.);for(var dy=-radius;dy<=radius;dy++){for(var dx=-radius;dx<=radius;dx++){if p.a.z!=0. || dx*dx+dy*dy<=squared{let c=oil_sample(q+vec2<i32>(dx,dy));if oil_bin(c,bins)==mode{sum+=vec4<f32>(c.rgb*c.a,c.a);}}}}
        destination[i]=sum/f32(max(1u,histogram[mode]));return;
    }
    if operation==37u{
        let center=raw(i);let radius=i32(p.a.x);let squared=((2*radius+1)*(2*radius+1)+2)/4;var below=vec4<f32>(0.);var area=0.;
        for(var dy=-radius;dy<=radius;dy++){for(var dx=-radius;dx<=radius;dx++){if dx*dx+dy*dy<=squared{below+=select(vec4<f32>(0.),vec4<f32>(1.),raw_mirror(q+vec2<i32>(dx,dy))<center);area+=1.;}}}
        let factor=-0.2*p.a.y*(1.-0.75*dot(center.rgb,vec3<f32>(0.299,0.587,0.114)));destination[i]=vec4<f32>(mix(center,below/area,factor).rgb,center.a);return;
    }
    if operation==38u{
        let c=raw(i);var seed=pixel_seed(u32(p.a.w)^(u32(p.b.z)|(u32(p.b.w)<<16u)),point);
        if next_float(&seed)>p.a.z/100.{destination[i]=c;return;}
        let r=i32(kernel[u32(next_float(&seed)*16384.)].x);let g=i32(kernel[u32(next_float(&seed)*16384.)].x);let b=i32(kernel[u32(next_float(&seed)*16384.)].x);
        let gray=(4899*r+9618*g+1867*b)>>14u;let sat=i32((p.a.y/100.)*4096.);let intensity=(p.a.x/100.)*100.;let dev=i32(intensity*intensity/4.);
        let colored=vec3<i32>(gray)+((vec3<i32>(r,g,b)-vec3<i32>(gray))*sat>>vec3<u32>(12u));
        let delta=(colored*dev+vec3<i32>(32768))>>vec3<u32>(16u);destination[i]=vec4<f32>((c.rgb*255.+vec3<f32>(delta))/255.,c.a);return;
    }
    if operation==39u{
        var sum=vec4<f32>(0.);for(var j=0u;j<arrayLength(&kernel);j++){
            let k=kernel[j];var seed=pixel_seed(u32(p.b.x)^(u32(k.x)|(u32(k.y)<<16u)),point);
            let angle=next_float(&seed)*6.2831855;let random_radius=next_float(&seed);let radius=pow(abs(k.z+random_radius*(k.w-k.z)),1./p.a.z);
            sum+=sample_at(point+radius*vec2<f32>(cos(angle),sin(angle)),2u);
        }
        destination[i]=sum/f32(arrayLength(&kernel));return;
    }
    if operation==46u{destination[i]=red_eye_color(q,raw(i));return;}
    if operation==45u{destination[i]=pixel_up(point);return;}
    if operation==52u{let at=(point-vec2<f32>(0.5))%vec2<f32>(p.a.z)+vec2<f32>(0.5);destination[i]=mix_effect(source[i],turbulence_color(at),u32(p.b.y),point);return;}
    if operation==43u || operation==44u{
        let quality=u32(select(p.a.y,p.b.y,operation==44u));var sum=vec4<f32>(0.);for(var j=0u;j<quality*quality;j++){let at=point+rgss(j,quality);var mapped=crystal_point(at);var edge=1u;if operation==44u{mapped=dents_point(at);edge=2u;}sum+=sample_at(mapped,edge);}destination[i]=sum/f32(quality*quality);return;
    }
    if operation==48u{
        let size=vec2<f32>(f32(p.width),f32(p.height));let angle=p.a.x*0.01745329252;let scale=max(cos(angle)+abs(sin(angle))*size.y/size.x,cos(angle)+abs(sin(angle))*size.x/size.y);let mapped=size*0.5+rotate((point-size*0.5)/scale,p.a.x);destination[i]=sample_mode(mapped,1u,u32(p.a.y));return;
    }
    if operation==49u{
        let delta=vec2<f32>(q*2)-vec2<f32>(f32(p.width),f32(p.height));var value=0.;var amplitude=1.;var divisor=u32(p.a.x);for(var j=0u;j<12u;j++){if amplitude<=0.03 || divisor==0u{break;}let at=vec2<f32>(65536.)+delta/f32(divisor);value+=perlin(at,(u32(p.a.w)+j)&255u)*amplitude;amplitude*=p.a.y;divisor>>=1u;}let color=mix(color24(p.b.x,p.b.y),color24(p.b.z,p.b.w),(value+1.)*0.5);destination[i]=mix_effect(source[i],premul(color),u32(p.a.z),point);return;
    }
    if operation==50u || operation==51u{
        let quality=u32(p.a.w);var color=vec4<f32>(0.);for(var j=0u;j<quality*quality;j++){color+=fractal_color(point+rgss(j,quality));}color/=f32(quality*quality);var mode=u32(p.b.x);if operation==51u{mode=u32(p.b.y);if p.b.x!=0.{color=vec4<f32>(vec3<f32>(color.a)-color.rgb,color.a);}}destination[i]=mix_effect(source[i],color,mode,point);return;
    }
    if operation==47u{
        let center=raw(i);var blur=center.rgb*0.36;for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){if x!=0 || y!=0{let v=sharpen_sample(point+vec2<f32>(f32(x),f32(y))*1.033);blur+=v*select(0.039731,0.120269,x==0 || y==0);}}}let delta=dot(center.rgb-blur,vec3<f32>(0.299,0.587,0.114));let threshold=select(1.,abs(delta)/max(p.a.y,1e-20),abs(delta)<p.a.y);destination[i]=premul(vec4<f32>(clamp(center.rgb+vec3<f32>(delta*p.a.x*threshold),vec3<f32>(0.),vec3<f32>(1.)),center.a));return;
    }
    if operation==53u{
        var gx=vec3<f32>(0.);var gy=vec3<f32>(0.);for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){let c=straight(tap(q+vec2<i32>(x,y),2u));let wx=f32(x)*select(1.,2.,y==0 && p.a.z==0.);let wy=f32(y)*select(1.,2.,x==0 && p.a.z==0.);gx+=c*wx;gy+=c*wy;}}let edge=clamp((gx*gx+gy*gy)*p.a.x,vec3<f32>(0.),vec3<f32>(1.));let c=raw(i);destination[i]=select(vec4<f32>(edge,1.),premul(vec4<f32>(c.rgb-edge,c.a)),p.a.w!=0.);return;
    }
    if operation>=23u && operation<=26u {
        let quality=u32(p.b.x);let count=quality*quality;var sum=vec4<f32>(0.);
        for(var j=0u;j<count;j++){
            var offset=vec2<f32>(0.);if quality>1u{let t=f32(j+1u)/f32(count+1u);offset=vec2<f32>(fract(t*f32(quality))-0.5,t-0.5);}
            let at=point+offset;var mapped=at;var edge=u32(p.a.w);
            // UI order Clamp, Wrap, Mirror, Transparent -> sampling mode.
            var mode=1u;if edge==1u{mode=3u;}else if edge==2u{mode=2u;}else if edge==3u{mode=0u;}
            let size=vec2<f32>(f32(p.width),f32(p.height));let radius=min(p.bounds.z,p.bounds.w)*0.5;
            var center=p.bounds.xy+p.bounds.zw*(vec2<f32>(1.)+p.a.yz)*0.5;
            if operation==24u{center=p.bounds.xy+p.bounds.zw*(vec2<f32>(1.)+p.a.zw)*0.5;mode=1u;}
            let delta=at-center;let distance=length(delta);
            if operation==23u{let k=1.-distance/radius;if k>0.{mapped=center+delta*(1.-p.a.x*k*k);}}
            else if operation==24u{let k=clamp(1.-distance/(p.a.y*radius),0.,1.);let amount=-p.a.x;let angle=atan2(delta.y,delta.x)+k*k*k*amount*amount*sign(amount)*0.01;mapped=center+distance*vec2<f32>(cos(angle),sin(angle));}
            else if operation==25u{let scale=p.a.x*min(p.bounds.z,p.bounds.w)/min(size.x,size.y);let r=min(size.x,size.y)*0.5;let square=dot(delta,delta);if square>0.{mapped=center+delta*mix(1.,r*r/square,scale);}else{mapped=center;}}
            else if operation==26u{let origin=size*0.5;let d=at-origin;let angle=-p.a.x*0.01745329252;let cs=cos(angle);let sn=sin(angle);var v=vec2<f32>(dot(d,vec2<f32>(cs,sn)),dot(d,vec2<f32>(-sn,cs)));v+=p.a.z*p.a.z*sign(p.a.z)*0.1*tan(v*3.14159265359/p.a.y);mapped=origin+vec2<f32>(dot(v,vec2<f32>(cs,-sn)),dot(v,vec2<f32>(sn,cs)));}
            sum+=sample_at(mapped,mode);
        }
        destination[i]=sum/f32(count);return;
    }
    var sum=vec4<f32>(0.);var weight=0.;
    if operation==20u {
        let count=u32(p.a.x);for(var j=0u;j<count;j++){let angle=(p.a.z-90.)*0.01745329252+f32(j)*6.28318530718/f32(count);let at=point+p.a.y*vec2<f32>(sin(angle),cos(angle));sum+=sample_at(at,0u);weight+=sample_coverage(at);}
    } else if operation==21u {
        let center=vec2<f32>(f32(p.width),f32(p.height))*(vec2<f32>(1.)+p.a.yz)*0.5;let delta=point-center;let count=u32(exp2(p.a.w+5.));
        for(var j=0u;j<count;j++){let t=f32(j)/f32(count-1u);let angle=-mix(-p.a.x*0.5,p.a.x*0.5,t)*0.01745329252;let at=center+vec2<f32>(delta.x*cos(angle)-delta.y*sin(angle),delta.x*sin(angle)+delta.y*cos(angle));sum+=sample_at(at,0u);weight+=sample_coverage(at);}
    } else if operation==22u {
        let center=vec2<f32>(f32(p.width),f32(p.height))*(vec2<f32>(1.)+p.a.zw)*0.5;let delta=point-center;let lo=min(1.,p.a.x);let hi=max(1.,p.a.x);let count=u32(exp2(p.b.x+5.));
        for(var j=0u;j<count;j++){let scale=mix(lo,hi,f32(j)/f32(count-1u));let sub=select(lo,hi,lo>=1.);let w=pow(abs((scale-sub)/(hi-lo)),p.a.y);sum+=sample_at(center+delta/scale,1u)*w;weight+=w;}
    }
    if weight>0.{sum/=weight;}destination[i]=sum;
}
