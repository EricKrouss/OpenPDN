// Recovered numeric permutation tables; image processing remains GPU resident.
const permutation:array<u32,256> = array<u32,256>(151u,160u,137u,91u,90u,15u,131u,13u,201u,95u,96u,53u,194u,233u,7u,225u,140u,36u,103u,30u,69u,142u,8u,99u,37u,240u,21u,10u,23u,190u,6u,148u,247u,120u,234u,75u,0u,26u,197u,62u,94u,252u,219u,203u,117u,35u,11u,32u,57u,177u,33u,88u,237u,149u,56u,87u,174u,20u,125u,136u,171u,168u,68u,175u,74u,165u,71u,134u,139u,48u,27u,166u,77u,146u,158u,231u,83u,111u,229u,122u,60u,211u,133u,230u,220u,105u,92u,41u,55u,46u,245u,40u,244u,102u,143u,54u,65u,25u,63u,161u,1u,216u,80u,73u,209u,76u,132u,187u,208u,89u,18u,169u,200u,196u,135u,130u,116u,188u,159u,86u,164u,100u,109u,198u,173u,186u,3u,64u,52u,217u,226u,250u,124u,123u,5u,202u,38u,147u,118u,126u,255u,82u,85u,212u,207u,206u,59u,227u,47u,16u,58u,17u,182u,189u,28u,42u,223u,183u,170u,213u,119u,248u,152u,2u,44u,154u,163u,70u,221u,153u,101u,155u,167u,43u,172u,9u,129u,22u,39u,253u,19u,98u,108u,110u,79u,113u,224u,232u,178u,185u,112u,104u,218u,246u,97u,228u,251u,34u,242u,193u,238u,210u,144u,12u,191u,179u,162u,241u,81u,51u,145u,235u,249u,14u,239u,107u,49u,192u,214u,31u,181u,199u,106u,157u,184u,84u,204u,176u,115u,121u,50u,45u,127u,4u,150u,254u,138u,236u,205u,93u,222u,114u,67u,29u,24u,72u,243u,141u,128u,195u,78u,66u,215u,61u,156u,180u);
const crystal_p0:array<u32,256> = array<u32,256>(91u,213u,94u,40u,203u,67u,124u,200u,62u,51u,223u,53u,208u,111u,31u,28u,137u,108u,37u,114u,74u,46u,177u,32u,71u,117u,187u,130u,191u,81u,45u,120u,178u,220u,241u,88u,216u,100u,82u,15u,230u,212u,87u,153u,243u,141u,72u,199u,174u,242u,197u,251u,103u,176u,33u,42u,34u,60u,229u,96u,142u,255u,9u,239u,5u,112u,139u,247u,131u,201u,70u,35u,22u,17u,146u,122u,217u,21u,50u,185u,39u,238u,202u,204u,140u,55u,8u,250u,173u,189u,52u,155u,127u,240u,136u,226u,193u,101u,147u,133u,1u,83u,76u,227u,165u,160u,222u,80u,38u,171u,86u,10u,12u,44u,59u,157u,56u,68u,110u,211u,102u,148u,73u,188u,159u,179u,169u,30u,145u,168u,175u,118u,143u,172u,190u,218u,164u,234u,27u,233u,84u,184u,214u,0u,106u,26u,135u,85u,192u,244u,170u,128u,252u,183u,29u,113u,7u,132u,69u,248u,246u,181u,196u,119u,3u,224u,121u,92u,104u,90u,237u,64u,19u,126u,123u,161u,182u,58u,75u,109u,24u,79u,99u,158u,249u,25u,125u,209u,138u,57u,206u,18u,156u,43u,236u,152u,245u,77u,254u,144u,162u,36u,221u,89u,253u,95u,98u,115u,48u,129u,195u,205u,47u,210u,207u,151u,61u,16u,134u,219u,78u,167u,225u,14u,150u,198u,163u,231u,105u,228u,154u,54u,23u,180u,194u,235u,65u,2u,107u,93u,149u,4u,97u,6u,66u,232u,186u,166u,20u,41u,13u,116u,215u,11u,49u,63u);
const crystal_p1:array<u32,256> = array<u32,256>(87u,48u,248u,240u,154u,176u,128u,190u,0u,110u,229u,55u,74u,226u,234u,159u,202u,35u,36u,251u,183u,45u,27u,42u,187u,100u,186u,23u,69u,250u,80u,231u,117u,57u,143u,125u,243u,174u,215u,97u,11u,66u,71u,121u,15u,138u,109u,92u,9u,133u,172u,33u,141u,178u,46u,41u,19u,207u,56u,60u,242u,107u,126u,5u,32u,253u,196u,20u,89u,115u,171u,26u,81u,198u,237u,124u,228u,165u,25u,58u,246u,102u,13u,194u,84u,50u,103u,199u,10u,112u,114u,44u,136u,167u,120u,166u,204u,70u,93u,18u,203u,95u,6u,161u,217u,152u,218u,205u,193u,181u,68u,118u,182u,1u,72u,150u,96u,189u,188u,244u,157u,255u,8u,233u,4u,137u,156u,164u,85u,78u,177u,169u,79u,7u,116u,160u,91u,122u,232u,195u,83u,151u,3u,90u,43u,249u,127u,134u,105u,221u,73u,22u,145u,104u,131u,210u,77u,34u,162u,86u,245u,52u,130u,132u,12u,163u,38u,191u,155u,175u,61u,235u,214u,252u,170u,14u,247u,192u,208u,17u,29u,40u,65u,135u,2u,173u,49u,230u,148u,180u,88u,64u,222u,153u,211u,39u,51u,146u,98u,149u,223u,209u,236u,206u,158u,99u,63u,140u,212u,111u,216u,147u,82u,139u,59u,224u,239u,254u,106u,94u,113u,47u,30u,227u,75u,241u,24u,54u,62u,142u,219u,119u,108u,37u,53u,101u,67u,179u,220u,185u,129u,184u,28u,21u,225u,200u,213u,76u,123u,238u,31u,16u,144u,201u,168u,197u);
const mix_modes:array<u32,46> = array<u32,46>(0u,1u,10u,1000u,1001u,1002u,1003u,1004u,1005u,1006u,1007u,1008u,1009u,1010u,1011u,1012u,1013u,1014u,1015u,1016u,1017u,1018u,1019u,1020u,1021u,1022u,1023u,1024u,1025u,2000u,2001u,2002u,2003u,2004u,2005u,2u,3u,4u,5u,6u,7u,9u,12u,2006u,2007u,8u);

// Improved Perlin noise, including the reference's second permutation lookup.
fn perm(i:u32)->u32{return permutation[i&255u];}
fn grad(h0:u32,v:vec2<f32>)->f32{let h=h0&15u;let u=select(v.y,v.x,h<8u);var w=0.;if h<4u{w=v.y;}else if h==12u || h==14u{w=v.x;}return select(-u,u,(h&1u)==0u)+select(-w,w,(h&2u)==0u);}
fn perlin(v:vec2<f32>,seed:u32)->f32{let f=floor(v);let t=v-f;let ij=vec2<u32>(vec2<i32>(f))&vec2<u32>(255u);let uv=t*t*t*(t*(t*6.-vec2<f32>(15.))+vec2<f32>(10.));let a=perm(ij.x+seed)+ij.y;let b=perm(ij.x+1u+seed)+ij.y;return mix(mix(grad(perm(perm(a)),t),grad(perm(perm(b)),t-vec2<f32>(1.,0.)),uv.x),mix(grad(perm(perm(a+1u)),t-vec2<f32>(0.,1.)),grad(perm(perm(b+1u)),t-vec2<f32>(1.)),uv.x),uv.y);}
fn premul(v:vec4<f32>)->vec4<f32>{return vec4<f32>(v.rgb*v.a,v.a);}
fn color24(rgb:f32,alpha:f32)->vec4<f32>{let v=u32(rgb);return vec4<f32>(f32(v>>16u),f32((v>>8u)&255u),f32(v&255u),alpha)/255.;}
fn rotate(v:vec2<f32>,angle:f32)->vec2<f32>{let a=angle*0.01745329252;return vec2<f32>(v.x*cos(a)-v.y*sin(a),v.x*sin(a)+v.y*cos(a));}
fn rgss(j:u32,quality:u32)->vec2<f32>{if quality==1u{return vec2<f32>(0.);}let t=f32(j+1u)/f32(quality*quality+1u);return vec2<f32>(fract(t*f32(quality))-0.5,t-0.5);}
fn cubic_weight(x0:f32)->f32{let x=abs(x0);if x<1.{return 1.5*x*x*x-2.5*x*x+1.;}if x<2.{return -0.5*x*x*x+2.5*x*x-4.*x+2.;}return 0.;}
fn sample_cubic(at:vec2<f32>,mode:u32)->vec4<f32>{let v=at-vec2<f32>(0.5);let base=vec2<i32>(floor(v));let t=fract(v);var s=vec4<f32>(0.);for(var y=-1;y<=2;y++){for(var x=-1;x<=2;x++){s+=tap(base+vec2<i32>(x,y),mode)*cubic_weight(f32(x)-t.x)*cubic_weight(f32(y)-t.y);}}return s;}
fn sample_mode(at:vec2<f32>,mode:u32,interpolation:u32)->vec4<f32>{if interpolation==2u{return tap(vec2<i32>(floor(at)),mode);}if interpolation==1u{return sample_at(at,mode);}return sample_cubic(at,mode);}
fn blend_lum(v:vec3<f32>)->f32{return dot(v,vec3<f32>(0.3,0.59,0.11));}
fn blend_sat(v:vec3<f32>)->f32{return max(v.r,max(v.g,v.b))-min(v.r,min(v.g,v.b));}
fn set_lum(c:vec3<f32>,l:f32)->vec3<f32>{var v=c+vec3<f32>(l-blend_lum(c));let n=min(v.r,min(v.g,v.b));let x=max(v.r,max(v.g,v.b));if n<0.{v=vec3<f32>(l)+(v-vec3<f32>(l))*l/(l-n);}if x>1.{v=vec3<f32>(l)+(v-vec3<f32>(l))*(1.-l)/(x-l);}return v;}
fn set_sat(c:vec3<f32>,s:f32)->vec3<f32>{let n=min(c.r,min(c.g,c.b));let x=max(c.r,max(c.g,c.b));if x==n{return vec3<f32>(0.);}return (c-vec3<f32>(n))*s/(x-n);}
fn burn(d:vec3<f32>,s:vec3<f32>)->vec3<f32>{return select(vec3<f32>(0.),vec3<f32>(1.)-min(vec3<f32>(1.),(vec3<f32>(1.)-d)/max(s,vec3<f32>(1e-20))),s>vec3<f32>(0.));}
fn dodge(d:vec3<f32>,s:vec3<f32>)->vec3<f32>{return select(min(vec3<f32>(1.),d/max(vec3<f32>(1.)-s,vec3<f32>(1e-20))),vec3<f32>(1.),s>=vec3<f32>(1.));}
fn mix_effect(dst:vec4<f32>,src:vec4<f32>,choice:u32,pos:vec2<f32>)->vec4<f32>{
 let mode=mix_modes[choice];switch mode{
 case 0u:{return src+dst*(1.-src.a);}case 1u:{return dst+src*(1.-dst.a);}case 2u:{return src*dst.a;}case 3u:{return dst*src.a;}case 4u:{return src*(1.-dst.a);}case 5u:{return dst*(1.-src.a);}case 6u:{return src*dst.a+dst*(1.-src.a);}case 7u:{return dst*src.a+src*(1.-dst.a);}case 8u:{return src*(1.-dst.a)+dst*(1.-src.a);}case 9u:{return min(src+dst,vec4<f32>(1.));}case 10u:{return src;}case 12u:{return dst*(1.-src.a);}default:{} }
 let d=clamp(straight(dst),vec3<f32>(0.),vec3<f32>(1.));let s=clamp(straight(src),vec3<f32>(0.),vec3<f32>(1.));var c=s;
 switch mode{
 case 1000u:{c=d*s;}case 1001u:{c=d+s-d*s;}case 1002u,2004u:{c=min(d,s);}case 1003u,2005u:{c=max(d,s);}
 case 1004u:{var seed=pixel_seed(0u,pos);return select(dst,vec4<f32>(s,1.),next_float(&seed)<src.a);}
 case 1005u:{c=burn(d,s);}case 1006u:{c=max(vec3<f32>(0.),d+s-vec3<f32>(1.));}case 1007u:{c=select(s,d,blend_lum(d)<blend_lum(s));}case 1008u:{c=select(s,d,blend_lum(d)>blend_lum(s));}case 1009u:{c=dodge(d,s);}case 1010u,2000u:{c=min(vec3<f32>(1.),d+s);}
 case 1011u:{c=select(2.*d*s,vec3<f32>(1.)-2.*(vec3<f32>(1.)-d)*(vec3<f32>(1.)-s),d>vec3<f32>(0.5));}
 case 1012u:{let g=select(((16.*d-vec3<f32>(12.))*d+vec3<f32>(4.))*d,sqrt(d),d>vec3<f32>(0.25));c=select(d-(vec3<f32>(1.)-2.*s)*d*(vec3<f32>(1.)-d),d+(2.*s-vec3<f32>(1.))*(g-d),s>vec3<f32>(0.5));}
 case 1013u:{c=select(2.*d*s,vec3<f32>(1.)-2.*(vec3<f32>(1.)-d)*(vec3<f32>(1.)-s),s>vec3<f32>(0.5));}
 case 1014u:{c=select(burn(d,2.*s),dodge(d,2.*s-vec3<f32>(1.)),s>vec3<f32>(0.5));}
 case 1015u:{c=clamp(d+2.*s-vec3<f32>(1.),vec3<f32>(0.),vec3<f32>(1.));}case 1016u:{c=select(min(d,2.*s),max(d,2.*s-vec3<f32>(1.)),s>vec3<f32>(0.5));}case 1017u:{let v=select(burn(d,2.*s),dodge(d,2.*s-vec3<f32>(1.)),s>vec3<f32>(0.5));c=select(vec3<f32>(0.),vec3<f32>(1.),v>=vec3<f32>(0.5));}
 case 1018u:{c=abs(d-s);}case 1019u:{c=d+s-2.*d*s;}case 1020u:{c=set_lum(set_sat(s,blend_sat(d)),blend_lum(d));}case 1021u:{c=set_lum(set_sat(d,blend_sat(s)),blend_lum(d));}case 1022u:{c=set_lum(s,blend_lum(d));}case 1023u:{c=set_lum(d,blend_lum(s));}case 1024u:{c=max(vec3<f32>(0.),d-s);}case 1025u:{c=min(vec3<f32>(1.),d/max(s,vec3<f32>(1e-20)));}
 case 2001u:{c=dodge(d*d,s);}case 2002u:{c=dodge(s*s,d);}case 2003u:{c=vec3<f32>(1.)-abs(vec3<f32>(1.)-d-s);}case 2006u,2007u:{let scale=select(255.,65535.,mode==2007u);c=vec3<f32>(vec3<u32>(round(d*scale))^vec3<u32>(round(s*scale)))/scale;}default:{} }
 return blend_premultiplied(dst,src,c);
}
fn crystal_feature(cell:vec2<i32>,seed:u32)->vec2<f32>{let x=u32(cell.x);let y=u32(cell.y);return vec2<f32>(f32(crystal_p1[(crystal_p0[(seed^x)&255u]^y)&255u]),f32(crystal_p0[(crystal_p1[(seed^x)&255u]^y)&255u]))/255.;}
fn crystal_point(at:vec2<f32>)->vec2<f32>{let v=at/p.a.x;let cell=vec2<i32>(v);let seed=u32(p.a.z)*1566083941u+u32(p.b.w);var best=vec2<f32>(cell)+crystal_feature(cell,seed);var distance=dot(v-best,v-best);let offsets=array<vec2<i32>,8>(vec2<i32>(-1,0),vec2<i32>(0,-1),vec2<i32>(1,0),vec2<i32>(0,1),vec2<i32>(-1,-1),vec2<i32>(1,1),vec2<i32>(-1,1),vec2<i32>(1,-1));for(var k=0u;k<8u;k++){let c=cell+offsets[k];let point=vec2<f32>(c)+crystal_feature(c,seed);let d=dot(v-point,v-point);if d<distance{distance=d;best=point;}}return vec2<f32>(0.5)+best*p.a.x;}
fn dents_point(at:vec2<f32>)->vec2<f32>{let k=kernel[0];let r=kernel[1];var xy=(at-p.bounds.xy-p.bounds.zw*0.5)*k.x;var total=0.;var amplitude=1.;var frequency=1.;var remaining=k.z;let seed=(u32(p.b.z)+u32(p.b.w))&255u;for(var j=0u;j<u32(ceil(k.z));j++){let rotated=vec2<f32>(xy.x*r.y-xy.y*r.z,xy.x*r.z+xy.y*r.y);total+=perlin(rotated*frequency,seed)*amplitude*min(1.,remaining);amplitude*=k.w;if amplitude<0.001{break;}frequency*=2.;remaining-=1.;xy=rotated+vec2<f32>(499.,506.);}let angle=k.y*total;return at+r.x*vec2<f32>(-sin(angle),cos(angle));}
fn fractal_color(at:vec2<f32>)->vec4<f32>{let size=vec2<f32>(f32(p.width),f32(p.height));let uv=rotate((2.*at-size)/size.y,p.a.z);var x=0.;var y=0.;var c=0.;var value=0.;if operation==50u{let aspect=size.y/size.x;x=(uv.x-uv.y*aspect)/p.a.y;y=(uv.y+uv.x*aspect)/p.a.y;loop{let norm=x*x+y*y;if c>=256. || norm>=10000.{break;}let nx=x*x-y*y+bitcast<f32>(1050673152u);y=2.*x*y+bitcast<f32>(1022739087u);x=nx;c+=1.;}let j=c-(2.-2.*log(10000.)/log(max(x*x+y*y,1e-20)));value=p.a.x*j/256.;return premul(clamp(vec4<f32>(value)-vec4<f32>(1.,2.,3.,0.),vec4<f32>(0.),vec4<f32>(1.)));}
let ri=uv/(1.+20.*max(0.0001,p.a.y))-vec2<f32>(bitcast<f32>(1060320051u),bitcast<f32>(1049918177u));loop{if c*p.a.x>=1024. || x*x+y*y>=100000.{break;}let nx=x*x-y*y+ri.x;y=2.*x*y+ri.y;x=nx;c+=1.;}let m=c-log(max(x*x+y*y,1e-20))/log(100000.);value=(64.+p.a.x*m)/256.;return premul(clamp(vec4<f32>(value)-vec4<f32>(3.,2.,1.,0.),vec4<f32>(0.),vec4<f32>(1.)));}
fn raw_at(at:vec2<f32>)->vec4<f32>{let v=at-vec2<f32>(0.5);let q=vec2<i32>(floor(v));let t=fract(v);return mix(mix(raw_mirror(q),raw_mirror(q+vec2<i32>(1,0)),t.x),mix(raw_mirror(q+vec2<i32>(0,1)),raw_mirror(q+vec2<i32>(1)),t.x),t.y);}
fn sketch_quantile(at:vec2<f32>,start:u32,count:u32,channel:u32)->f32{
 var markers:array<f32,5>;var positions:array<i32,5>;let probability=p.a.y/100.;
 for(var j=0u;j<5u;j++){markers[j]=raw_at(at+kernel[start+j].xy)[channel];}
 for(var hi=4u;hi>0u;hi--){for(var lo=0u;lo<hi;lo++){if markers[lo]>markers[hi]{let old=markers[lo];markers[lo]=markers[hi];markers[hi]=old;}}}
 let sorted=markers;positions[0]=0;positions[1]=i32(round(2.*probability));positions[2]=i32(round(4.*probability));positions[3]=i32(round(2.+2.*probability));positions[4]=4;
 for(var j=1u;j<4u;j++){markers[j]=sorted[u32(positions[j])];}
 let multipliers=array<f32,5>(0.,probability*0.5,probability,(1.+probability)*0.5,1.);
 for(var j=5u;j<count;j++){
  let x=raw_at(at+kernel[start+j].xy)[channel];if x<markers[0]{markers[0]=x;}else if x>=markers[4]{markers[4]=x;}
  for(var m=1u;m<4u;m++){if x<markers[m]{positions[m]+=1;}}positions[4]+=1;
  for(var index=1u;index<4u;index++){
   let m=select(4u-index,index,probability>=0.5);let n0=positions[m-1u];let n=positions[m];let n1=positions[m+1u];let d=f32(j)*multipliers[m]-f32(n);
   if (d>=1. && n1-n>1) || (d<= -1. && n0-n< -1){
    let di=select(-1,1,d>0.);let fd=f32(di);let q0=markers[m-1u];let v=markers[m];let q1=markers[m+1u];
    let parabola=v+fd/f32(n1-n0)*(f32(n-n0+di)*(q1-v)/f32(n1-n)+f32(n1-n-di)*(v-q0)/f32(n-n0));
    let neighbor=u32(i32(m)+di);let line=v+fd*(markers[neighbor]-v)/f32(positions[neighbor]-n);
    markers[m]=select(line,parabola,q0<parabola && parabola<q1);positions[m]+=di;
   }
  }
 }
 return markers[2];
}
fn turbulence_perm(i:u32)->u32{return u32(kernel[2u+(i&255u)].x);}
fn turbulence_grad(x:u32,y:u32,delta:vec2<f32>)->vec4<f32>{let index=2u*turbulence_perm(turbulence_perm(x)+y);return kernel[258u+index]*delta.x+kernel[259u+index]*delta.y;}
fn turbulence_color(at:vec2<f32>)->vec4<f32>{
 var xy=(at-vec2<f32>(0.5))*kernel[0].xy;var stitch=vec2<u32>(kernel[1].xy);var wrap=vec2<u32>(kernel[1].zw);var total=vec4<f32>(0.);var amplitude=1.;
 for(var octave=0u;octave<u32(p.a.x);octave++){
  let shifted=xy+vec2<f32>(4096.);let base=vec2<u32>(shifted);let t=fract(shifted);var lo=base;var hi=base+vec2<u32>(1u);lo=select(lo,lo-stitch,lo>=wrap);hi=select(hi,hi-stitch,hi>=wrap);let fade=t*t*(vec2<f32>(3.)-2.*t);
  let row0=mix(turbulence_grad(lo.x,lo.y,t),turbulence_grad(hi.x,lo.y,t-vec2<f32>(1.,0.)),fade.x);
  let row1=mix(turbulence_grad(lo.x,hi.y,t-vec2<f32>(0.,1.)),turbulence_grad(hi.x,hi.y,t-vec2<f32>(1.)),fade.x);let value=mix(row0,row1,fade.y);
  total+=select(abs(value),value,p.a.w==1.)*amplitude;amplitude*=0.5;xy*=2.;stitch*=2u;wrap=wrap*2u-vec2<u32>(4096u);
 }
 if p.a.w==1.{total=total*0.5+vec4<f32>(0.5);}return premul(clamp(total,vec4<f32>(0.),vec4<f32>(1.)));
}
fn pixel_down(at:vec2<f32>)->vec4<f32>{
 let center=p.bounds.xy+p.bounds.zw*(vec2<f32>(1.)+p.a.yz)*0.5;let original_at=center+(at-center)*p.a.x;let mode=u32(p.a.w);
 if mode==5u{return sample_mode(original_at,1u,2u);}if mode==4u{return sample_at(original_at,1u);}if mode==3u{return sample_cubic(original_at,1u);}
 // Integrate the scaled footprint. Linear multisampling uses a regular grid;
 // the cubic and anisotropic choices filter the same footprint differently.
 let samples=min(64u,u32(ceil(p.a.x)));var sum=vec4<f32>(0.);
 for(var y=0u;y<samples;y++){for(var x=0u;x<samples;x++){let offset=(vec2<f32>(f32(x)+0.5,f32(y)+0.5)/f32(samples)-vec2<f32>(0.5))*p.a.x;let point=original_at+offset;if mode==1u{sum+=sample_cubic(point,1u);}else{sum+=sample_at(point,1u);}}}return sum/f32(samples*samples);
}
fn pixel_cache(q:vec2<i32>)->vec4<f32>{let size=vec2<i32>(kernel[0].zw);let at=clamp(q,vec2<i32>(0),size-vec2<i32>(1));return source[u32(at.y*size.x+at.x)];}
fn pixel_up(at:vec2<f32>)->vec4<f32>{let center=p.bounds.xy+p.bounds.zw*(vec2<f32>(1.)+p.a.yz)*0.5;let v=center+(at-center)/p.a.x-kernel[0].xy-vec2<f32>(0.5);let base=vec2<i32>(floor(v));let t=fract(v);if p.b.x==2.{return pixel_cache(vec2<i32>(floor(v+vec2<f32>(0.5))));}if p.b.x==1.{return mix(mix(pixel_cache(base),pixel_cache(base+vec2<i32>(1,0)),t.x),mix(pixel_cache(base+vec2<i32>(0,1)),pixel_cache(base+vec2<i32>(1)),t.x),t.y);}var sum=vec4<f32>(0.);for(var y=-1;y<=2;y++){for(var x=-1;x<=2;x++){sum+=pixel_cache(base+vec2<i32>(x,y))*cubic_weight(f32(x)-t.x)*cubic_weight(f32(y)-t.y);}}return sum;}
fn diffusion_error(q:vec2<i32>,part:u32)->vec3<i32>{if any(q<vec2<i32>(0)) || any(q>=vec2<i32>(i32(p.width),i32(p.height))){return vec3<i32>(0);}let e=vec3<i32>(auxiliary[u32(q.y)*p.width+u32(q.x)].rgb);if part==0u{return e*7>>vec3<u32>(4u);}if part==1u{return e*5>>vec3<u32>(4u);}if part==2u{return e*3>>vec3<u32>(4u);}return e-(e*7>>vec3<u32>(4u))-(e*5>>vec3<u32>(4u))-(e*3>>vec3<u32>(4u));}
fn quantize_pixel(q:vec2<i32>,i:u32){
 let bytes=vec4<u32>(round(raw(i)*255.));var c=vec3<i32>(bytes.rgb);let transparent=bytes.a<u32(p.a.w);
 if transparent{c=vec3<i32>(0);}else if bytes.a!=255u{c=vec3<i32>((bytes.rgb*bytes.a+vec3<u32>(255u*(255u-bytes.a)))/255u);}
 if p.a.z!=0.{let error=diffusion_error(q-vec2<i32>(1,0),0u)+diffusion_error(q-vec2<i32>(0,1),1u)+diffusion_error(q+vec2<i32>(1,-1),2u)+diffusion_error(q-vec2<i32>(1,1),3u);c=clamp(c-(error*i32(p.a.z)>>vec3<u32>(3u)),vec3<i32>(0),vec3<i32>(255));}
 let colors=u32(kernel[0].x);let opaque=u32(kernel[0].y);let stride=u32(kernel[0].z);
 var best=0u;var distance=2147483647;
 if transparent && colors>opaque {best=opaque;}else{
  let cube=u32(c.r>>4)|(u32(c.g>>4)<<4u)|(u32(c.b>>4)<<8u);
  for(var j=0u;j<opaque;j++) {
   let k=u32(kernel[1u+colors+cube*stride+j/4u][j%4u]);
   let delta=vec3<i32>(kernel[1u+k].rgb)-c;let d=delta.r*delta.r+delta.g*delta.g+delta.b*delta.b;
   if d<=distance {distance=d;best=k;}
  }
 }
 let result=kernel[1u+best];destination[i]=premul(result/255.);auxiliary[i]=vec4<f32>(result.rgb-vec3<f32>(c),0.);
}
fn eye_sector(delta:vec2<f32>)->u32{
 var angle=0.;if delta.x==0.{angle=select(90.,270.,delta.y<0.);}else{
  angle=atan(delta.y/delta.x)*57.295779513;
  if delta.y<0.{angle+=select(180.,360.,angle<0.);}else if angle<0.{angle+=180.;}
 }return min(59u,u32(floor(angle/6.)));
}
fn red_eye_color(q:vec2<i32>,c:vec4<f32>)->vec4<f32>{
 let point=vec2<f32>(q);var color=c.rgb*255.;
 if c.a==0.{return c;}
 for(var area=0u;area<arrayLength(&kernel);area+=64u){
  let bounds=kernel[area];if any(point<bounds.xy) || any(point>=bounds.zw){continue;}
  let data=kernel[area+1u];let extra=kernel[area+2u];let stats=kernel[area+3u];let delta=point-data.xy;let distance=length(delta);let radius=data.z;if distance>=radius || radius<=0.{continue;}
  var rgb=color;
  if data.w!=0.{var falloff=radius/5.;if falloff<1.{falloff=2.;}let max_distance=length(data.xy-extra.xy);var pivot=extra.xy;if max_distance>=radius/3.{pivot=stats.zw;}let r=length(point-pivot);let brightness=max(0.,1.-(r*r/falloff)/falloff);let t=length(point-stats.zw)-radius;let geometric=1.-((2.*t*t)/radius)/radius;rgb*=max(brightness,geometric);}
  if rgb.r<=rgb.g || rgb.r<=rgb.b{continue;}
  let bin=eye_sector(delta);let polar=kernel[area+4u+bin];let comp=(rgb.b*2.+rgb.g*9.)/11.;var cap=255.;
  if polar.y!=0.{cap=polar.x;if comp<polar.y*2.{cap=(((comp-polar.y)*(comp-polar.y))/(polar.y*(-2.)*polar.y)+1.)*cap;}}
  var spatial=(distance/radius)/3.+0.667;if extra.z!=0.{spatial=((distance-radius)*(distance-radius)*extra.w)/(radius*radius)+1.;}spatial*=polar.x;
  var highlight=255.;if stats.x-stats.y>40. && length(data.xy-extra.xy)<radius*0.667{highlight=dot(point-extra.xy,point-extra.xy)*5.;}
  let allowed=min(highlight,min(spatial,cap));let base=max(rgb.g,rgb.b);if allowed<rgb.r-base{
   let add=((rgb.r-base)-allowed)*5./16.;let g=rgb.g+add;let b=rgb.b+add;let r=select(g,b,rgb.g<rgb.b)+allowed;color=floor(vec3<f32>(r,g,b)+vec3<f32>(0.5));
  }
 }
 return vec4<f32>(color/255.,c.a);
}

// Native Sharpen samples an unassociated input image before interpolation.
fn sharpen_tap(at:vec2<i32>)->vec3<f32>{let c=raw_mirror(at);return select(vec3<f32>(0.),c.rgb,c.a>0.);}
fn sharpen_sample(point:vec2<f32>)->vec3<f32>{let at=point-vec2<f32>(0.5);let q=vec2<i32>(floor(at));let t=fract(at);return mix(mix(sharpen_tap(q),sharpen_tap(q+vec2<i32>(1,0)),t.x),mix(sharpen_tap(q+vec2<i32>(0,1)),sharpen_tap(q+vec2<i32>(1)),t.x),t.y);}
