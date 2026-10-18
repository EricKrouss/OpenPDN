using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.Loader;
if(args.Length!=2) throw new ArgumentException("Usage: adjustment-probe EXTRACTED-X64 OUTPUT.bin");
string folder=Path.GetFullPath(args[0]);
AssemblyLoadContext.Default.Resolving+=(_,name)=>{string path=Path.Combine(folder,name.Name+".dll");return File.Exists(path)?AssemblyLoadContext.Default.LoadFromAssemblyPath(path):null;};
var assembly=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Windows.Framework.dll"));
var type=assembly.GetType("PaintDotNet.Direct2D1.Effects.PosterizeEffect2+Impl",true)!;
var generate=type.GetMethod("GenerateLevels",BindingFlags.NonPublic|BindingFlags.Instance)!.CreateDelegate<Generate>(RuntimeHelpers.GetUninitializedObject(type));
var table=new List<byte>();
for(int count=2;count<=64;count++) {
 var values=new float[256];generate(count,values);
 table.AddRange(values.Select(v=>(byte)MathF.Round(v*255f)));
}
File.WriteAllBytes(args[1],table.ToArray());
Console.WriteLine($"Wrote {table.Count} original PosterizeEffect2 table entries");
delegate void Generate(int count,Span<float> table);
