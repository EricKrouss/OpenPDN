using System.Reflection;
using System.Runtime.Loader;
using System.Text.Json;
using System.Reflection.Metadata;
using System.Reflection.PortableExecutable;
if(args.Length is <2 or >3) throw new ArgumentException("Usage: filter-probe EXTRACTED-X64 OUTPUT.json [noise|tables|bokeh|sketch|octree|proximity]");
string folder=Path.GetFullPath(args[0]);
AssemblyLoadContext.Default.Resolving+=(_,name)=>{string path=Path.Combine(folder,name.Name+".dll");return File.Exists(path)?AssemblyLoadContext.Default.LoadFromAssemblyPath(path):null;};
if(args.Length==3 && args[2]=="proximity") {
 var primitives=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Primitives.dll"));var frameworkQuant=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Framework.dll"));
 var colorType=primitives.GetType("PaintDotNet.Imaging.ColorBgra32",true)!;var tableType=frameworkQuant.GetType("PaintDotNet.Imaging.PaletteTable",true)!;var mapType=frameworkQuant.GetType("PaintDotNet.Imaging.Quantization.PaletteMap",true)!;
 var createPaletteMap=mapType.GetMethod("Create")!;var find=mapType.GetMethod("FindClosestPaletteIndex")!;
 var all=new List<object>();
 foreach(int count in new[]{2,3,16,17,32,256}) {
  var random=new Random(count);var palette=new List<int[]>();var native=Array.CreateInstance(colorType,count);
  for(int i=0;i<count;i++){int[] c=i==0?[0,0,0,255]:i==1?[2,0,0,255]:[random.Next(256),random.Next(256),random.Next(256),255];palette.Add(c);native.SetValue(Activator.CreateInstance(colorType,[(byte)c[2],(byte)c[1],(byte)c[0],(byte)255]),i);}
  var table=Activator.CreateInstance(tableType,[native])!;var map=createPaletteMap.Invoke(null,[table]);var targets=new List<object>();
  for(int i=0;i<4096;i++){int[] c=i==0?[1,0,0,255]:[random.Next(256),random.Next(256),random.Next(256),255];var target=Activator.CreateInstance(colorType,[(byte)c[2],(byte)c[1],(byte)c[0],(byte)255]);targets.Add(new {color=c,index=(byte)find.Invoke(map,[target])!});}
  all.Add(new {palette,targets});
 }
 File.WriteAllText(args[1],JsonSerializer.Serialize(all));Console.WriteLine("Wrote 24576 original palette proximity queries");return;
}
if(args.Length==3 && args[2]=="octree") {
 var primitives=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Primitives.dll"));
 var frameworkQuant=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Framework.dll"));
 var colorType=primitives.GetType("PaintDotNet.Imaging.ColorBgr24",true)!;
 var treeType=frameworkQuant.GetType("PaintDotNet.Imaging.Quantization.OctreeQuantizer+Octree",true)!;
 var fundamentals=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Fundamentals.dll"));
 var cancel=fundamentals.GetType("PaintDotNet.Threading.CancellationTokenUtil",true)!.GetProperty("False")!.GetValue(null);
 var add=treeType.GetMethod("AddColor")!;var reduce=treeType.GetMethod("Reduce")!;var gather=treeType.GetMethod("GatherColors")!;
 var casesOct=new List<object>();
 foreach(int seed in Enumerable.Range(0,8)) foreach(int limit in new[]{1,2,3,7,16,32}) {
  var random=new Random(seed);var tree=Activator.CreateInstance(treeType,true)!;
  var entries=new List<object>();
  // Distinct powers of two make every subtree population unique. Hash ties in
  // the original intentionally use a process-randomized .NET hash seed.
  for(int n=0;n<40;n++) { byte r=(byte)random.Next(256),g=(byte)random.Next(256),b=(byte)random.Next(256);long count=1L<<n;
   entries.Add(new {color=new int[]{r,g,b,255},count});add.Invoke(tree,[Activator.CreateInstance(colorType,[b,g,r]),count]);
  }
  reduce.Invoke(tree,[limit,cancel]);var list=(System.Collections.IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(colorType))!;gather.Invoke(tree,[list,cancel]);
  var palette=list.Cast<object>().Select(v=>new int[]{(byte)colorType.GetProperty("R")!.GetValue(v)!,(byte)colorType.GetProperty("G")!.GetValue(v)!,(byte)colorType.GetProperty("B")!.GetValue(v)!,(byte)255}).ToArray();
  casesOct.Add(new {seed,limit,entries,palette});
 }
 File.WriteAllText(args[1],JsonSerializer.Serialize(casesOct));Console.WriteLine($"Wrote {casesOct.Count} original Octree palettes");return;
}
if(args.Length==3 && args[2]=="noise") {
 var gpu=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Effects.Gpu.dll"));
 var impl=gpu.GetType("PaintDotNet.Effects.Gpu.PdnAddNoiseEffect+Impl",true)!;
 var method=impl.GetMethods(BindingFlags.NonPublic|BindingFlags.Static).Single(m=>m.Name=="CreateLookupTable" && m.GetParameters()[0].ParameterType==typeof(Span<int>));
 var createNoise=method.CreateDelegate<CreateNoiseTable>();
 int[] lookup=new int[16384];createNoise(lookup);
 var random=gpu.GetType("PaintDotNet.Effects.Gpu.HlslRandom",true)!;
 var hash=random.GetMethod("PcgHash",BindingFlags.Public|BindingFlags.Static)!;
 var next=random.GetMethod("PcgNextFloat",BindingFlags.Public|BindingFlags.Static)!;
 var hashes=Enumerable.Range(0,4096).Select(i=>{
  uint input=unchecked((uint)i*2654435761u);uint output=(uint)hash.Invoke(null,[input])!;
  object[] arguments=[input];float value=(float)next.Invoke(null,arguments)!;
  return new {input,output,next_seed=(uint)arguments[0],next_float=value};
 }).ToArray();
 File.WriteAllText(args[1],JsonSerializer.Serialize(new {lookup_table=lookup,hashes}));
 Console.WriteLine($"Wrote {lookup.Length} noise samples and {hashes.Length} PCG cases from original managed IL");
 return;
}
if(args.Length==3 && args[2]=="sketch") {
 var sketchCases=new List<object>();
 foreach(float radius in new[]{0.25f,2.5f,25f,100f}) foreach(int iteration in new[]{0,1,2,19}) {
  int area=Math.Max(5,(int)Math.Round(Math.PI*(double)(radius*radius)));int count=Math.Clamp((int)Math.Round((float)area*.01f),5,area);var random=new Random(iteration);var offsets=new List<float[]>();
  for(int i=0;i<count;i++){double r=Math.Sqrt(random.NextDouble());double angle=random.NextDouble()*Math.Tau;offsets.Add([(float)(r*Math.Cos(angle)*radius),(float)(r*Math.Sin(angle)*radius)]);}
  sketchCases.Add(new {radius,iteration,offsets});
 }
 File.WriteAllText(args[1],JsonSerializer.Serialize(sketchCases));Console.WriteLine($"Wrote {sketchCases.Count} original runtime Sketch Blur offset streams");return;
}
var framework=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Windows.Framework.dll"));
if(args.Length==3 && args[2]=="tables") {
 using var pe=new PEReader(File.OpenRead(Path.Combine(folder,"PaintDotNet.Effects.Core.dll")));
 var metadata=pe.GetMetadataReader();
 var field=metadata.FieldDefinitions.Select(h=>metadata.GetFieldDefinition(h)).Single(f=>metadata.GetString(f.Name)=="EA3CF748EF2EED46C8D386654894EFB4A0ACA29E79789C2D3A4A27B57352CECD4");
 var bytes=pe.GetSectionData(field.GetRelativeVirtualAddress()).GetContent(0,1024).ToArray();
 int[] permutation=Enumerable.Range(0,256).Select(i=>BitConverter.ToInt32(bytes,i*4)).ToArray();
 var gpu=Assembly.LoadFrom(Path.Combine(folder,"PaintDotNet.Effects.Gpu.dll"));
 var crystal=gpu.GetType("PaintDotNet.Effects.Gpu.PdnCrystalizeSampleMap",true)!;
 var p0=(byte[])crystal.GetField("P0",BindingFlags.NonPublic|BindingFlags.Static)!.GetValue(null)!;
 var p1=(byte[])crystal.GetField("P1",BindingFlags.NonPublic|BindingFlags.Static)!.GetValue(null)!;
 var bokeh=framework.GetType("PaintDotNet.Direct2D1.Effects.BokehBlurEffect",true)!;
 var components=(System.Collections.IEnumerable)bokeh.GetProperty("KernelComponents",BindingFlags.NonPublic|BindingFlags.Static)!.GetValue(null)!;
 var parameters=components.Cast<System.Collections.IEnumerable>().Select(a=>a.Cast<object>().Select(v=>new[]{"X","Y","Z","W"}.Select(n=>Convert.ToDouble(v.GetType().GetProperty(n)!.GetValue(v))).ToArray()).ToArray()).ToArray();
 var mix=framework.GetType("PaintDotNet.Direct2D1.Effects.MixModeUtil",true)!;
 var modes=((System.Collections.IEnumerable)mix.GetMethod("GetUserMixModes")!.Invoke(null,null)!).Cast<object>().Select(v=>new {name=v.ToString(),value=Convert.ToInt32(v)}).ToArray();
 File.WriteAllText(args[1],JsonSerializer.Serialize(new {permutation,p0,p1,bokeh_parameters=parameters,mix_modes=modes}));
 Console.WriteLine($"Recovered Perlin/Crystalize tables, {parameters.Length} Bokeh component sets and {modes.Length} mix modes");return;
}
if(args.Length==3 && args[2]=="bokeh") {
 var impl=framework.GetType("PaintDotNet.Direct2D1.Effects.BokehBlurEffect+Impl",true)!;
 var method=impl.GetMethod("CreateKernelDataFloat32",BindingFlags.NonPublic|BindingFlags.Static)!;
 var kernels=new List<object>();
 foreach(int quality in Enumerable.Range(1,10)) foreach(double radius in new[]{0.25,2.5,25.0}) {
  var tuple=method.Invoke(null,[radius,quality])!;
  var components=(Array)tuple.GetType().GetField("Item4")!.GetValue(tuple)!;
  kernels.Add(new {radius,quality,kernels=components.Cast<object>().Select(v=>new {real=(float[])v.GetType().GetField("Item1")!.GetValue(v)!,imaginary=(float[])v.GetType().GetField("Item2")!.GetValue(v)!}).ToArray()});
 }
 File.WriteAllText(args[1],JsonSerializer.Serialize(kernels));Console.WriteLine($"Wrote {kernels.Count} original Bokeh kernels");return;
}
var implementation=framework.GetType("PaintDotNet.Direct2D1.Effects.GaussianBlurConvolutionEffect+Impl",true)!;
var create=implementation.GetMethod("CreateGaussianBlurKernels",BindingFlags.NonPublic|BindingFlags.Static)!;
var cases=new List<object>();
foreach(double sigma in new[]{0.01,0.1,0.5,2.0/3.0,2.25,10.0,100.0}) {
 var pair=create.Invoke(null,[sigma,6.0])!;
 var matrix=(double[,])pair.GetType().GetField("Item1")!.GetValue(pair)!;
 cases.Add(new{sigma,weights=Enumerable.Range(0,matrix.GetLength(1)).Select(i=>(float)matrix[0,i]).ToArray()});
}
File.WriteAllText(args[1],JsonSerializer.Serialize(cases));
Console.WriteLine($"Wrote {cases.Count} Gaussian point kernels from original managed IL");
delegate void CreateNoiseTable(Span<int> destination);
