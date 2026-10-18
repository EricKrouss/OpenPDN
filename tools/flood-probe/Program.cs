using System.Reflection;
using System.Runtime.Loader;
using System.Text.Json;
if(args.Length!=2) throw new ArgumentException("Usage: flood-probe EXTRACTED-X64 OUTPUT.json");
string folder=Path.GetFullPath(args[0]);
AssemblyLoadContext.Default.Resolving+=(_,name)=>{string path=Path.Combine(folder,name.Name+".dll");return File.Exists(path)?AssemblyLoadContext.Default.LoadFromAssemblyPath(path):null;};
var core=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Core.dll"));
var ui=Assembly.LoadFrom(Path.Combine(folder,"paintdotnet.dll"));
var colorType=core.GetType("PaintDotNet.ColorBgra",true)!;
var color=colorType.GetMethod("FromBgra",[typeof(byte),typeof(byte),typeof(byte),typeof(byte)])!;
object Color(byte[] p)=>color.Invoke(null,[p[2],p[1],p[0],p[3]])!;
var algorithm=ui.GetType("PaintDotNet.Tools.FloodFill.FloodFillAlgorithm",true)!;
var methods=new[]{"GetDistancePremultiplied","GetDistanceStraight"}.Select(n=>algorithm.GetMethod(n,BindingFlags.Static|BindingFlags.NonPublic)!).ToArray();
var random=new Random(5112);
var cases=new List<object>();
byte[] edges=[0,1,63,127,128,191,254,255];
for(int i=0;i<4096;i++) {
 byte[] a=[(byte)random.Next(256),(byte)random.Next(256),(byte)random.Next(256),edges[i%8]],b=[(byte)random.Next(256),(byte)random.Next(256),(byte)random.Next(256),edges[(i/8)%8]];
 if(i%17==0) b=(byte[])a.Clone();
 var expected=methods.Select(m=>(byte)m.Invoke(null,[Color(a),Color(b)])!).ToArray();
 cases.Add(new{a=a.Select(v=>(int)v).ToArray(),b=b.Select(v=>(int)v).ToArray(),premultiplied=expected[0],straight=expected[1]});
}
File.WriteAllText(args[1],JsonSerializer.Serialize(cases));
Console.WriteLine($"Wrote {cases.Count} paired distances from original FloodFillAlgorithm");
