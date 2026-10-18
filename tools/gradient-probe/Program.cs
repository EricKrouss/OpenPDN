using System.Reflection;
using System.Runtime.Loader;
using System.Text.Json;
if(args.Length!=2) throw new ArgumentException("Usage: gradient-probe EXTRACTED-X64 OUTPUT.json");
string folder=Path.GetFullPath(args[0]);
AssemblyLoadContext.Default.Resolving+=(_,name)=>{string path=Path.Combine(folder,name.Name+".dll");return File.Exists(path)?AssemblyLoadContext.Default.LoadFromAssemblyPath(path):null;};
var core=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Core.dll"));
var ui=Assembly.LoadFrom(Path.Combine(folder,"paintdotnet.dll"));
var pointType=ui.GetType("PaintDotNet.Rendering.GradientShapes+LinearStraight",true)!.GetConstructors()[0].GetParameters()[0].ParameterType;
object Point(float x,float y)=>Activator.CreateInstance(pointType,[x,y])!;
string[] names=["LinearStraight","LinearReflected","LinearDiamond","Radial","Conical","Spiral","SpiralCCW"];
var geometry=new List<object>();
for(int shape=0;shape<7;shape++) for(int repeat=0;repeat<3;repeat++) {
 string name=names[shape];
 if(shape==4 && repeat==0) name="ConicalNoRepeat";
 if(shape==5 && repeat==2) name="SpiralReflected";
 if(shape==6 && repeat==2) name="SpiralReflectedCCW";
 var type=ui.GetType("PaintDotNet.Rendering.GradientShapes+"+name,true)!;
 var instance=Activator.CreateInstance(type,[Point(2,3),Point(7,4)])!;
 var compute=type.GetMethod("ComputeLerp")!;
 string[] repeaters=["NoRepeat","RepeatWrapped","RepeatReflected"];
 var rt=ui.GetType("PaintDotNet.Rendering.GradientRepeaters+"+repeaters[repeat],true)!;
 var ri=Activator.CreateInstance(rt,true)!;var bound=rt.GetMethod("BoundLerp")!;
 var values=new List<float>();
 for(int y=0;y<11;y++) for(int x=0;x<17;x++) {float t=(float)compute.Invoke(instance,[(float)x,(float)y])!;values.Add((float)bound.Invoke(ri,[t])!);}
 geometry.Add(new{shape,repeat,values});
}
var colorType=core.GetType("PaintDotNet.ColorBgra",true)!;
var from=colorType.GetMethod("FromBgra",[typeof(byte),typeof(byte),typeof(byte),typeof(byte)])!;
var blend=colorType.GetMethod("Blend",[colorType,colorType,typeof(byte)])!;
object Color(int[] c)=>from.Invoke(null,[(byte)c[2],(byte)c[1],(byte)c[0],(byte)c[3]])!;
int[] Rgba(object c)=>new[]{"R","G","B","A"}.Select(n=>(int)(byte)colorType.GetField(n)!.GetValue(c)!).ToArray();
var colors=new List<object>();
foreach(int alphaA in new[]{0,1,127,255}) foreach(int alphaB in new[]{0,1,128,255}) {
 int[] a=[10,87,201,alphaA],b=[223,32,51,alphaB];
 var values=new List<int[]>();
 for(int weight=0;weight<256;weight++) values.Add(Rgba(blend.Invoke(null,[Color(a),Color(b),(byte)weight])!));
 colors.Add(new{a,b,values});
}
File.WriteAllText(args[1],JsonSerializer.Serialize(new{geometry,colors}));
Console.WriteLine($"Wrote {geometry.Count*187} original gradient coordinates and {colors.Count*256} original color interpolations");
