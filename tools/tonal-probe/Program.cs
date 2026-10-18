using System.Collections;
using System.Reflection;
using System.Runtime.Loader;
using System.Text.Json;
if(args.Length!=2) throw new ArgumentException("Usage: tonal-probe EXTRACTED-X64 OUTPUT.json");
string folder=Path.GetFullPath(args[0]);
AssemblyLoadContext.Default.Resolving+=(_,name)=>{string path=Path.Combine(folder,name.Name+".dll");return File.Exists(path)?AssemblyLoadContext.Default.LoadFromAssemblyPath(path):null;};
var primitives=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Primitives.dll"));
var gpu=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Effects.Gpu.dll"));
var levelsType=gpu.GetType("PaintDotNet.Effects.Gpu.LevelsParameters",true)!;
var ctor=levelsType.GetConstructors().Single(c=>c.GetParameters().Length==5);
var colorType=ctor.GetParameters()[0].ParameterType;
object Color(float[] c)=>Activator.CreateInstance(colorType,[c[0],c[1],c[2]])!;
float[] Components(object c)=>new[]{"R","G","B"}.Select(n=>(float)colorType.GetProperty(n)!.GetValue(c)!).ToArray();
var levels=new List<object>();
var random=new Random(12345);
for(int n=0;n<18;n++) {
 float[] black=n==0?[0,0,0]:Enumerable.Range(0,3).Select(_=>(float)random.NextDouble()*.4f).ToArray();
 float[] white=n==0?[1,1,1]:Enumerable.Range(0,3).Select(_=>.6f+(float)random.NextDouble()*.4f).ToArray();
 float[] gamma=n==0?[1,1,1]:Enumerable.Range(0,3).Select(_=>.1f+(float)random.NextDouble()*9.9f).ToArray();
 float[] ob=n==0?[0,0,0]:Enumerable.Range(0,3).Select(_=>(float)random.NextDouble()).ToArray();
 float[] ow=n==0?[1,1,1]:Enumerable.Range(0,3).Select(_=>(float)random.NextDouble()).ToArray();
 if(n==17) {black=[.5f,.7f,.9f];white=[.5f,.3f,.1f];}
 object instance=ctor.Invoke([Color(black),Color(white),Color(gamma),Color(ob),Color(ow)]);
 var table=((IEnumerable)levelsType.GetMethod("CreateLookupTable")!.Invoke(instance,[256])!).Cast<object>().Select(Components).ToArray();
 levels.Add(new{input_black=black,input_white=white,gamma,output_black=ob,output_white=ow,table});
}
var interpolator=primitives.GetType("PaintDotNet.Rendering.FunctionInterpolator",true)!;
var pointType=primitives.GetType("PaintDotNet.Rendering.Point2Double",true)!;
var create=interpolator.GetMethods().Single(m=>m.Name=="Create" && m.GetParameters()[1].ParameterType.GetGenericArguments()[0]==pointType);
var mode=Enum.Parse(create.GetParameters()[0].ParameterType,"Spline");
int[][][] points=[[[0,0],[255,255]],[[0,255],[64,180],[128,50],[200,90],[255,0]],[[0,0],[1,200],[2,20],[200,240],[255,255]],[[20,40],[80,130],[190,210],[230,100]]];
var curves=new List<object>();
foreach(var p in points) {
 var array=Array.CreateInstance(pointType,p.Length);for(int i=0;i<p.Length;i++) array.SetValue(Activator.CreateInstance(pointType,[p[i][0]/255.0,p[i][1]/255.0]),i);
 var instance=create.Invoke(null,[mode,array])!;var sample=instance.GetType().GetMethod("Sample")!;
 var values=Enumerable.Range(0,1024).Select(i=>(float)(double)sample.Invoke(instance,[i/1023.0])!).ToArray();
 curves.Add(new{points=p,values});
}
File.WriteAllText(args[1],JsonSerializer.Serialize(new{levels,curves}));
Console.WriteLine($"Wrote {levels.Count*256} original RGB Levels outputs and {curves.Count*1024} original spline samples");
