using System.Collections;
using System.Reflection;
using System.Reflection.Emit;
using System.Runtime.InteropServices;
using System.Runtime.Loader;
using System.Text.Json;

if (args.Length < 3) throw new ArgumentException("Usage: pdn-probe EXTRACTED-X64 HEAP-SHIM OUTPUT [verify|legacy|spline|constraints|text|overwrite|picker|brush|selection|palettes|resolution]");
string folder=Path.GetFullPath(args[0]);
nint shim=NativeLibrary.Load(Path.GetFullPath(args[1]));
AssemblyLoadContext.Default.Resolving += (_,name) => {
    string path=Path.Combine(folder,name.Name+".dll");
    if (!File.Exists(path)) return null;
    Assembly loaded=AssemblyLoadContext.Default.LoadFromAssemblyPath(path);
    if (name.Name=="TerraFX.Interop.Windows") {
        DllImportResolver heapResolver=(library,assembly,searchPath)=>library.Contains("kernel32",StringComparison.OrdinalIgnoreCase)?shim:0;
        var windowType=loaded.GetType("TerraFX.Interop.Windows.Windows")!;
        var resolverField=windowType.GetField("ResolveLibrary",BindingFlags.Static|BindingFlags.NonPublic)!;
        resolverField.SetValue(null,Delegate.Combine((Delegate?)resolverField.GetValue(null),heapResolver));
    }
    if (name.Name!.StartsWith("PaintDotNet"))
        NativeLibrary.SetDllImportResolver(loaded,(library,assembly,searchPath) => library.Contains("kernel32",StringComparison.OrdinalIgnoreCase) ? shim : 0);
    return loaded;
};
if (args.Length >= 4 && args[3] == "palettes") {
    const BindingFlags flags=BindingFlags.Static|BindingFlags.NonPublic;
    var type=Assembly.Load("paintdotnet").GetType("PaintDotNet.AppModel.PalettesService",true)!;
    object instance=System.Runtime.CompilerServices.RuntimeHelpers.GetUninitializedObject(type);
    var create=type.GetMethod("CreateDefaultPalette",flags)!;
    var parse=type.GetMethod("ParsePaletteString")!;
    int[][] colors(object values) => ((IEnumerable)values).Cast<object>().Select(c=>new[]{"R","G","B","A"}.Select(n=>Convert.ToInt32(c.GetType().GetProperty(n)!.GetValue(c))).ToArray()).ToArray();
    var defaults=colors(create.Invoke(null,null)!);
    var paddedCases=new List<object>();
    var ensureSize=type.GetMethod("EnsureValidPaletteSize")!;
    var paletteColorType=ensureSize.GetParameters()[0].ParameterType.GetElementType()!;
    foreach(int count in new[]{0,1,2,95,96,112}) {
        var valuesArray=Array.CreateInstance(paletteColorType,count);
        for(int i=0;i<count;i++) valuesArray.SetValue(paletteColorType.GetMethod("FromUInt32")!.Invoke(null,[(uint)((i*37)<<24|(i*17%256)<<16|(i*31%256)<<8|i*43%256)]),i);
        paddedCases.Add(new{input=colors(valuesArray),result=colors(ensureSize.Invoke(instance,[valuesArray])!)});
    }
    string[] values=["", "; comment\n", "FF010203", "010203", "#FF010203", "0xFF010203", "F", "ABC", "01234567", "123456789", "-1", "+123", "FFFFFFFF", "  ffAB01CD  ; label", ";header\n00ABCDEF\nwrong\nFF000001", "GG000000", "-0", "0X00FF00", "0x+12", "+0xFF", "000000000000000FF", "FF000001\rFF000002", "\uFEFFFF010203", "\n\r\tFF00FF00\t\r\n"];
    var cases=new List<object>();
    foreach(string input in values.Concat(new[]{string.Join('\n',Enumerable.Range(0,112).Select(i=>$"FF{i:X6}"))})) cases.Add(new{input,result=colors(parse.Invoke(instance,[input])!)});
    var rgbType=Assembly.Load("paintdotnet").GetType("PaintDotNet.Rendering.Int32RgbColor",true)!;
    var hsvType=Assembly.Load("paintdotnet").GetType("PaintDotNet.Rendering.Int32HsvColor",true)!;
    int[] triplet(object value,string[] names)=>names.Select(n=>(int)value.GetType().GetProperty(n)!.GetValue(value)!).ToArray();
    var random=new Random(0x51_12);var rgbCases=new List<object>();var hsvCases=new List<object>();
    for(int n=0;n<1024;n++) {
        int[] rgb=[random.Next(256),random.Next(256),random.Next(256)];
        object r=Activator.CreateInstance(rgbType,rgb.Cast<object>().ToArray())!;
        rgbCases.Add(new{rgb,hsv=triplet(rgbType.GetMethod("ToHsv")!.Invoke(r,null)!,["Hue","Saturation","Value"])});
        int[] hsv=[n<361?n:random.Next(361),n%101,(n/3)%101];
        object h=Activator.CreateInstance(hsvType,hsv.Cast<object>().ToArray())!;
        hsvCases.Add(new{hsv,rgb=triplet(hsvType.GetMethod("ToRgb")!.Invoke(h,null)!,["Red","Green","Blue"])});
    }
    var wheelType=Assembly.Load("paintdotnet").GetType("PaintDotNet.Controls.ColorWheelControl",true)!;
    const BindingFlags instanceFlags=BindingFlags.Instance|BindingFlags.NonPublic;
    var originalGrab=wheelType.GetMethods(instanceFlags).Single(m=>m.Name=="GrabColor" && m.GetParameters().Length==4);
    var fields=new Dictionary<FieldInfo,MethodInfo>();
    foreach(var (field,method) in new[]{("wheelCenterOffset","Center"),("wheelRadius","Radius"),("snap","Snap")}) fields.Add(wheelType.GetField(field,instanceFlags)!,typeof(WheelOracle).GetMethod(method)!);
    var grab=IlOracle.Clone(originalGrab,fields:fields);
    var xyType=originalGrab.GetParameters()[0].ParameterType;
    var originalColor=wheelType.GetMethod("FromHueSaturationAndAlpha")!;
    WheelOracle.ColorMethod=IlOracle.Clone(originalColor);
    var pixel=IlOracle.Clone(wheelType.GetMethod("GetWheelPixelColor",instanceFlags)!,originalColor,typeof(WheelOracle).GetMethod("Color")!.MakeGenericMethod(originalColor.ReturnType));
    var wheelCases=new List<object>();var wheelPixels=new List<object>();
    for(int n=0;n<512;n++) {
        int[] point=[random.Next(-20,157),random.Next(-20,157)],basis=[random.Next(361),random.Next(101),n%101];
        bool snap=n%2==0,lockHue=n%4==0,lockRadius=n%4==1;
        WheelOracle.SnapValue=snap;
        var hsv=grab.Invoke(null,[null,Activator.CreateInstance(xyType,point.Cast<object>().ToArray()),Activator.CreateInstance(hsvType,basis.Cast<object>().ToArray()),lockHue,lockRadius])!;
        wheelCases.Add(new{point,basis,snap,lockHue,lockRadius,result=triplet(hsv,["Hue","Saturation","Value"])});
        double dx=random.NextDouble()*140-70,dy=random.NextDouble()*140-70;
        var c=pixel.Invoke(null,[null,68,dx,dy])!;
        wheelPixels.Add(new{point=new[]{dx,dy},result=new[]{"R","G","B","A"}.Select(name=>Convert.ToInt32(c.GetType().GetProperty(name)!.GetValue(c))).ToArray()});
    }
    string fileText=";header\rFF010203\n80405060\r\n";
    var fileCases=new List<object>();
    foreach(var encoding in new System.Text.Encoding[]{new System.Text.UTF8Encoding(true),System.Text.Encoding.Unicode,System.Text.Encoding.BigEndianUnicode,System.Text.Encoding.UTF32,new System.Text.UTF32Encoding(true,true)}) {
        byte[] data=encoding.GetPreamble().Concat(encoding.GetBytes(fileText)).ToArray();
        using var reader=new StreamReader(new MemoryStream(data),System.Text.Encoding.UTF8);
        fileCases.Add(new{bytes=data.Select(b=>(int)b).ToArray(),result=colors(parse.Invoke(instance,[reader.ReadToEnd()])!)});
    }
    foreach(byte[] data in new byte[][]{[0x3b,0xFF,0x0A,0x46,0x46,0x30,0x31,0x30,0x32,0x30,0x33], [0xFF,0xFE,0x3B,0x00,0x00,0xD8], [0xFF,0xFE,0x00,0x00,0x00,0xD8,0x00,0x00]}) {
        using var reader=new StreamReader(new MemoryStream(data),System.Text.Encoding.UTF8);
        fileCases.Add(new{bytes=data.Select(b=>(int)b).ToArray(),result=colors(parse.Invoke(instance,[reader.ReadToEnd()])!)});
    }
    File.WriteAllText(args[2],JsonSerializer.Serialize(new{defaults,cases,rgbCases,hsvCases,wheelCases,wheelPixels,fileCases,paddedCases}));
    Console.WriteLine($"Wrote {defaults.Length} original default colors and {cases.Count} palette parser cases");return;
}
if (args.Length >= 4 && args[3] == "brush") {
    const BindingFlags flags=BindingFlags.Static|BindingFlags.NonPublic;
    var assembly=Assembly.Load("paintdotnet");
    var basic=assembly.GetType("PaintDotNet.Brushes.Basic.BasicSampledBrushRenderer",true)!;
    var sampled=assembly.GetType("PaintDotNet.Brushes.SampledBrushRenderer",true)!;
    var path=assembly.GetType("PaintDotNet.Brushes.BrushStrokeSampleWetList",true)!;
    var smooth=assembly.GetType("PaintDotNet.Brushes.SmoothedBrushInputPointList",true)!;
    var flow=sampled.GetMethod("GetFlowOpacity",flags,null,[typeof(double)],null)!;
    var soft=basic.GetMethod("GetSoftDiameter",flags)!;
    var pressure=path.GetMethod("ConvertPressureToScale",flags,null,[typeof(double)],null)!;
    var generate=smooth.GetMethod("GeneratePoint",flags)!;
    var pointType=generate.GetParameters()[0].ParameterType.GenericTypeArguments[0];
    var renderer=System.Runtime.CompilerServices.RuntimeHelpers.GetUninitializedObject(assembly.GetType("PaintDotNet.Brushes.Basic.SampledCircleBrushRenderer",true)!);
    var sprite=basic.GetMethod("GetSpriteRenderInfo",BindingFlags.Instance|BindingFlags.NonPublic)!;
    var sampleType=sprite.GetParameters()[0].ParameterType;
    var centerType=sampleType.GetConstructors().Single().GetParameters()[0].ParameterType;
    sampled.GetProperty("Antialiased")!.SetValue(renderer,false);sampled.GetProperty("Flow")!.SetValue(renderer,1.0);
    var random=new Random(0x51_12); var plans=new List<object>(); var curves=new List<object>();var aliased=new List<object>();
    for(int n=0;n<256;n++) {
        double size=new[]{1.0,1.25,1.5,2.0,12.0,32.0,99.5,2000.0}[n%8];
        double hardness=n/255.0, f=(255-n)/255.0, p=n<8?new[]{-1.0,0.0,1.0/128,0.25,0.5,1.0,1.5,2.0}[n]:random.NextDouble();
        plans.Add(new{size,hardness,flow=f,pressure=p,softDiameter=(double)soft.Invoke(null,[size,hardness])!,opacity=(double)flow.Invoke(null,[f])!,scale=(double)pressure.Invoke(null,[p])!});
        double scale=(double)pressure.Invoke(null,[p])!,spacing=new[]{0.01,0.15,0.5,1.0,5.0}[n%5];
        double[] center=[(n-128)/4.0,(128-n)/4.0];
        basic.BaseType!.BaseType!.GetProperty("Size")!.SetValue(renderer,size);sampled.GetProperty("SpacingPercent")!.SetValue(renderer,spacing);
        object info=sprite.Invoke(renderer,[Activator.CreateInstance(sampleType,[Activator.CreateInstance(centerType,center.Cast<object>().ToArray()),scale])])!;
        object bounds=info.GetType().GetProperty("RenderBounds")!.GetValue(info)!;
        double[] rectangle=new[]{"X","Y","Width","Height"}.Select(c=>(double)bounds.GetType().GetProperty(c)!.GetValue(bounds)!).ToArray();
        aliased.Add(new{size,scale,spacing,center,bounds=rectangle,pixels=(double)info.GetType().GetProperty("SpacingPixels")!.GetValue(info)!});
    }
    for(int n=0;n<128;n++) {
        double[][] input=Enumerable.Range(0,4).Select(i=>new[]{random.NextDouble()*100-50,random.NextDouble()*100-50,random.NextDouble()}).ToArray();
        var points=Array.CreateInstance(pointType,4);
        for(int i=0;i<4;i++) points.SetValue(Activator.CreateInstance(pointType,input[i].Cast<object>().ToArray()),i);
        float t=n<3?new[]{0f,0.5f,1f}[n]:(float)random.NextDouble();
        object output=generate.Invoke(null,[points,1,t,0.0,0.5])!;
        var position=pointType.GetProperty("Position")!.GetValue(output)!;
        double[] result=[(double)position.GetType().GetProperty("X")!.GetValue(position)!, (double)position.GetType().GetProperty("Y")!.GetValue(position)!, (double)pointType.GetProperty("Pressure")!.GetValue(output)!];
        curves.Add(new{input,t,result});
    }
    File.WriteAllText(args[2],JsonSerializer.Serialize(new{plans,curves,aliased}));
    Console.WriteLine($"Wrote {plans.Count} original brush plans, {aliased.Count} aliased stamps and {curves.Count} smoothing cases"); return;
}
if (args.Length >= 4 && args[3] == "picker") {
    var type=Assembly.Load("paintdotnet").GetType("PaintDotNet.Tools.ColorPickerTool",true)!;
    var method=type.GetMethods(BindingFlags.Static|BindingFlags.NonPublic).Single(m=>m.Name=="GetAverageColor" && m.GetParameters().Length==1 && m.GetParameters()[0].ParameterType.IsGenericType && m.GetParameters()[0].ParameterType.GetGenericTypeDefinition()==typeof(ReadOnlySpan<>));
    var pickerColorType=method.GetParameters()[0].ParameterType.GenericTypeArguments[0];
    var create=pickerColorType.GetMethod("FromBgra",[typeof(byte),typeof(byte),typeof(byte),typeof(byte)])!;
    var run=typeof(PickerOracle).GetMethod("Run")!.MakeGenericMethod(pickerColorType);
    var random=new Random(0x51_12);var cases=new List<object>();
    for(int n=0;n<256;n++) {
        int count=new[]{0,1,2,3,9,25,121,961,2601}[n%9];
        var input=new List<int[]>();var colors=Array.CreateInstance(pickerColorType,count);
        for(int i=0;i<count;i++) {
            int[] c=[random.Next(256),random.Next(256),random.Next(256),n%5==0?0:n%5==1?255:random.Next(256)];
            input.Add(c);colors.SetValue(create.Invoke(null,[(byte)c[2],(byte)c[1],(byte)c[0],(byte)c[3]]),i);
        }
        object result=run.Invoke(null,[method,colors])!;
        int[] value=new[]{"R","G","B","A"}.Select(c=>Convert.ToInt32(pickerColorType.GetProperty(c)!.GetValue(result))).ToArray();
        cases.Add(new{input,result=value});
    }
    File.WriteAllText(args[2],JsonSerializer.Serialize(cases));Console.WriteLine($"Wrote {cases.Count} original average-color cases");return;
}
if (args.Length >= 4 && args[3] == "overwrite") {
    var type=Assembly.Load("PaintDotNet.Core").GetType("PaintDotNet.ColorBgra",true)!;
    var create=type.GetMethod("FromBgra",[typeof(byte),typeof(byte),typeof(byte),typeof(byte)])!;
    var method=type.GetMethod("Overwrite",[type,type,typeof(byte)])!;
    var random=new Random(0x51_12); var cases=new List<object>();
    for(int n=0;n<4096;n++) {
        byte[] a=new byte[4],b=new byte[4]; random.NextBytes(a); random.NextBytes(b);
        if(n<1024) { a[3]=(byte)(n%4==0?0:n%4==1?255:a[3]); b[3]=(byte)(n%4<2?0:255); }
        byte coverage=(byte)(n<1024?n/4:random.Next(256));
        object color(byte[] c)=>create.Invoke(null,[c[2],c[1],c[0],c[3]])!;
        object result=method.Invoke(null,[color(a),color(b),coverage])!;
        byte[] value=new[]{"R","G","B","A"}.Select(c=>(byte)type.GetField(c)!.GetValue(result)!).ToArray();
        cases.Add(new{dst=a.Select(c=>(int)c).ToArray(),src=b.Select(c=>(int)c).ToArray(),coverage,result=value.Select(c=>(int)c).ToArray()});
    }
    File.WriteAllText(args[2],JsonSerializer.Serialize(cases)); Console.WriteLine($"Wrote {cases.Count} original overwrite cases"); return;
}
if (args.Length >= 4 && args[3] == "constraints") {
    var type=Assembly.Load("paintdotnet").GetType("PaintDotNet.Tools.Shapes.ShapesToolChanges",true)!;
    var method=type.GetMethod("ConstrainPoints15Degrees",BindingFlags.Static|BindingFlags.NonPublic)!;
    var pointType=method.GetParameters()[0].ParameterType;
    var random=new Random(0x51_12);
    var cases=new List<object>();
    for(int n=0;n<256;n++) {
        double[] a=n<64?new[]{0.5,-17.5}:new[]{random.NextDouble()*2000-1000,random.NextDouble()*2000-1000};
        double angle=(n-32)*Math.PI/24.0;
        double[] b=n<64?new[]{a[0]+100.5*Math.Cos(angle),a[1]+100.5*Math.Sin(angle)}:new[]{random.NextDouble()*2000-1000,random.NextDouble()*2000-1000};
        object?[] values=[Activator.CreateInstance(pointType,[a[0],a[1]]),Activator.CreateInstance(pointType,[b[0],b[1]]),null];
        method.Invoke(null,values);
        double[] endpoint=new[]{"X","Y"}.Select(axis=>(double)pointType.GetProperty(axis)!.GetValue(values[2])!).ToArray();
        double theta=Math.Atan2(b[1]-a[1],b[0]-a[0]);
        double scaled=12.0*theta/Math.PI;
        double rounded=Math.Round(scaled,MidpointRounding.AwayFromZero);
        cases.Add(new{start=a,end=b,endpoint,theta,scaled,rounded});
    }
    File.WriteAllText(args[2],JsonSerializer.Serialize(cases));
    Console.WriteLine($"Wrote {cases.Count} original line constraints");
    return;
}
if (args.Length >= 4 && args[3] == "text") {
    var type=Assembly.Load("paintdotnet").GetType("PaintDotNet.Tools.Text.TextTool",true)!;
    const BindingFlags fields=BindingFlags.Instance|BindingFlags.NonPublic;
    var random=new Random(0x51_12);
    var cases=new List<object>();
    string[] operations=["PerformLeft","PerformRight","PerformControlLeft","PerformControlRight","PerformControlBackspace","PerformControlDelete"];
    string[] samples=["", "a", "word  next", "one... two!", "(hello)  world", "  space  ", "a+b=$c", "café—déjà  vu", "αβ, γδ", "中文、文字", "a\tb", "a_b", "a²Ⅲb"];
    for(int n=0;n<512;n++) {
        string[] lines=[samples[random.Next(samples.Length)],samples[random.Next(samples.Length)],samples[random.Next(samples.Length)]];
        int line=random.Next(lines.Length),offset=random.Next(lines[line].Length+1);
        string operation=operations[n%operations.Length];
        object instance=System.Runtime.CompilerServices.RuntimeHelpers.GetUninitializedObject(type);
        var linesField=type.GetField("lines",fields)!;
        object pooled=Activator.CreateInstance(linesField.FieldType)!;
        foreach(string value in lines)linesField.FieldType.GetMethod("Add",[typeof(string)])!.Invoke(pooled,[value]);
        linesField.SetValue(instance,pooled);
        type.GetField("linePos",fields)!.SetValue(instance,line);
        type.GetField("textPos",fields)!.SetValue(instance,offset);
        type.GetMethod(operation,fields)!.Invoke(instance,null);
        cases.Add(new{lines,line,offset,operation,result=((IEnumerable)linesField.GetValue(instance)!).Cast<string>().ToArray(),resultLine=(int)type.GetField("linePos",fields)!.GetValue(instance)!,resultOffset=(int)type.GetField("textPos",fields)!.GetValue(instance)!});
        ((IDisposable)pooled).Dispose();
    }
    File.WriteAllText(args[2],JsonSerializer.Serialize(cases));
    Console.WriteLine($"Wrote {cases.Count} original text editing cases");
    return;
}
if (args.Length >= 4 && args[3] == "spline") {
    var framework=Assembly.Load("PaintDotNet.Windows.Framework");
    var type=framework.GetType("PaintDotNet.Rendering.BezierSplineUtil",true)!;
    var method=type.GetMethods(BindingFlags.Public|BindingFlags.Static).Single(m=>m.Name=="ConvertCardinalSplineToBeziers" && m.GetParameters()[0].ParameterType.IsGenericType && m.GetParameters()[0].ParameterType.GetGenericTypeDefinition()==typeof(IReadOnlyList<>));
    var pointType=method.GetParameters()[0].ParameterType.GetGenericArguments()[0];
    var random=new Random(0x51_12);
    var cases=new List<object>();
    for(int n=0;n<64;n++) {
        double[][] knots=Enumerable.Range(0,4).Select(i=>n==0?new[]{i*10.0,i*5.0}:n==1?new[]{12.0,12.0}:n==2?new[]{i%2*100.0,0.0}:new[]{random.NextDouble()*20000-10000,random.NextDouble()*20000-10000}).ToArray();
        var points=Array.CreateInstance(pointType,4);
        for(int i=0;i<4;i++)points.SetValue(Activator.CreateInstance(pointType,[knots[i][0],knots[i][1]]),i);
        var result=method.Invoke(null,[points])!;
        var segments=((IEnumerable)result).Cast<object>().Select(segment=>new[]{"Point1","Point2","Point3"}.Select(name=> {
            var p=segment.GetType().GetProperty(name)!.GetValue(segment)!;
            return new[]{"X","Y"}.Select(axis=>(double)pointType.GetProperty(axis)!.GetValue(p)!).ToArray();
        }).ToArray()).ToArray();
        cases.Add(new{knots,segments});
        (result as IDisposable)?.Dispose();
    }
    File.WriteAllText(args[2],JsonSerializer.Serialize(cases));
    Console.WriteLine($"Wrote {cases.Count} original four-knot natural splines");
    return;
}
Type TypeOf(string name) => new[]{"PaintDotNet.Data","PaintDotNet.Core","PaintDotNet.Fundamentals"}
    .Select(n=>Assembly.Load(n).GetType(name)).First(t=>t!=null)!;
void Set(object o,string name,object value) => o.GetType().GetProperty(name)!.SetValue(o,value);
object Get(object o,string name) => o.GetType().GetProperty(name)!.GetValue(o)!;
var docType=TypeOf("PaintDotNet.Document");
docType.GetMethod("Initialize",BindingFlags.Static|BindingFlags.NonPublic)!.Invoke(null,[new Version(5,112,9563,32325)]);
var layerType=TypeOf("PaintDotNet.BitmapLayer");
var colorType=TypeOf("PaintDotNet.ColorBgra");
if (args.Length>=4 && args[3]=="resolution") {
    var document=Activator.CreateInstance(docType,[32,24])!;
    var unit=docType.GetProperty("DpuUnit")!;
    unit.SetValue(document,Enum.Parse(unit.PropertyType,"Inch"));
    docType.GetProperty("DpuX")!.SetValue(document,144.0);
    docType.GetProperty("DpuY")!.SetValue(document,120.0);
    var metadata=docType.GetProperty("Metadata")!.GetValue(document)!;
    var entries=metadata.GetType().GetField("entries",BindingFlags.Instance|BindingFlags.NonPublic)!.GetValue(metadata)!;
    var convert=docType.GetMethod("ConvertMeasurement",[typeof(double),unit.PropertyType,unit.PropertyType,typeof(double),unit.PropertyType])!;
    var cases=new List<object>();
    for(int source=1;source<=3;source++)for(int basis=1;basis<=3;basis++)for(int result=1;result<=3;result++)foreach(double dpu in new[]{0.01,1.0,37.5,96.0,144.0,65536.0}) {
        if(basis==1 && dpu!=1.0) continue;
        double length=123.456;
        cases.Add(new{length,source,basis,dpu,result,value=convert.Invoke(null,[length,Enum.ToObject(unit.PropertyType,source),Enum.ToObject(unit.PropertyType,basis),dpu,Enum.ToObject(unit.PropertyType,result)])});
    }
    var writes=new List<object>();
    foreach(int u in new[]{1,2,3})foreach(double value in new[]{1.0,37.79527559055118,96.0,144.125,65536.0}) {
        if(u==1 && value!=1.0)continue;
        object d=Activator.CreateInstance(docType,[8,6])!;
        unit.SetValue(d,Enum.ToObject(unit.PropertyType,u));
        docType.GetProperty("DpuX")!.SetValue(d,value);
        docType.GetProperty("DpuY")!.SetValue(d,value);
        var m=docType.GetProperty("Metadata")!.GetValue(d)!;
        writes.Add(new{unit=u,value,metadata=m.GetType().GetField("entries",BindingFlags.Instance|BindingFlags.NonPublic)!.GetValue(m)});
    }
    var constrainerType=Assembly.Load("paintdotnet").GetType("PaintDotNet.Dialogs.ImageSizeDialog+ResizeConstrainer",true)!;
    object state(object c)=>new{width=constrainerType.GetProperty("NewWidth")!.GetValue(c),height=constrainerType.GetProperty("NewHeight")!.GetValue(c),pixelWidth=constrainerType.GetProperty("NewPixelWidth")!.GetValue(c),pixelHeight=constrainerType.GetProperty("NewPixelHeight")!.GetValue(c),units=Convert.ToInt32(constrainerType.GetProperty("Units")!.GetValue(c)),resolution=constrainerType.GetProperty("Resolution")!.GetValue(c),aspect=constrainerType.GetProperty("ConstrainToAspect")!.GetValue(c)};
    var resizeCases=new List<object>();var random=new Random(0x51_1296);
    for(int n=0;n<128;n++) {
        int w=random.Next(1,4097),h=random.Next(1,4097);
        object c=Activator.CreateInstance(constrainerType,[new System.Drawing.Size(w,h)])!;
        var operations=new List<object>();var states=new List<object>{state(c)};
        void apply(string property,object value) {
            var p=constrainerType.GetProperty(property)!;
            p.SetValue(c,p.PropertyType.IsEnum?Enum.ToObject(p.PropertyType,value):value);
            operations.Add(new{property,value});states.Add(state(c));
        }
        apply("ConstrainToAspect",n%2==0);
        apply("Resolution",random.NextDouble()*600+0.01);
        apply("NewPixelWidth",random.NextDouble()*4096+1);
        apply("NewHeight",random.NextDouble()*16+0.01);
        apply("Units",3);
        apply("NewWidth",random.NextDouble()*40+0.01);
        apply("Resolution",random.NextDouble()*200+0.01);
        double scale=random.NextDouble()*3+0.01;
        constrainerType.GetMethod("SetByPercent")!.Invoke(c,[scale]);
        operations.Add(new{property="SetByPercent",value=scale});states.Add(state(c));
        apply("Units",2);apply("Units",1);
        resizeCases.Add(new{original=new[]{w,h},operations,states});
    }
    File.WriteAllText(args[2],JsonSerializer.Serialize(new{metadata=entries,cases,writes,resizeCases}));
    Console.WriteLine($"Wrote {cases.Count} original conversions, {writes.Count} metadata writes and {resizeCases.Count} resize traces");return;
}
if (args.Length>=4 && args[3]=="selection") {
    var app=Assembly.Load("paintdotnet");
    var toolType=app.GetType("PaintDotNet.Tools.RectangleSelectTool",true)!;
    var tool=System.Runtime.CompilerServices.RuntimeHelpers.GetUninitializedObject(toolType);
    var document=Activator.CreateInstance(docType,[800,600])!;
    SelectionOracle.DocumentValue=document;
    var getter=app.GetType("PaintDotNet.Tools.Tool",true)!.GetProperty("Document",BindingFlags.Instance|BindingFlags.NonPublic)!.GetMethod!;
    var substitute=typeof(SelectionOracle).GetMethod("Document")!.MakeGenericMethod(docType);
    var normalMethod=toolType.GetMethod("CreateNormalRect",BindingFlags.Instance|BindingFlags.NonPublic)!;
    var normal=IlOracle.Clone(normalMethod,getter,substitute);
    var ratio=IlOracle.Clone(toolType.GetMethod("CreateFixedRatioRect",BindingFlags.Instance|BindingFlags.NonPublic)!,getter,substitute);
    var fixedMethod=toolType.GetMethod("CreateFixedSizeRect",BindingFlags.Instance|BindingFlags.NonPublic)!;
    var fixedSize=IlOracle.Clone(fixedMethod,getter,substitute);
    var pointType=normalMethod.GetParameters()[0].ParameterType;
    object pt(double[] p)=>Activator.CreateInstance(pointType,p.Cast<object>().ToArray())!;
    double[] rect(object r)=>new[]{"X","Y","Width","Height"}.Select(n=>(double)r.GetType().GetProperty(n)!.GetValue(r)!).ToArray();
    var random=new Random(0x51_12);var cases=new List<object>();
    for(int n=0;n<256;n++) {
        double[] a=[random.Next(-100,900),random.Next(-100,700)],b=[random.Next(-100,900),random.Next(-100,700)];
        double[] size=new[]{new[]{400.0,300.0},new[]{1.0,1.0},new[]{16.0,9.0},new[]{1000.5,800.75},new[]{0.25,2.5}}[n%5];
        cases.Add(new{start=a,end=b,size,shift=n%2==0,normal=rect(normal.Invoke(null,[tool,pt(a),pt(b),n%2==0])!),ratio=rect(ratio.Invoke(null,[tool,pt(a),pt(b),size[0],size[1]])!),fixedSize=rect(fixedSize.Invoke(null,[tool,pt(b),size[0],size[1],Enum.Parse(fixedMethod.GetParameters()[3].ParameterType,"Pixel")])!)});
    }
    File.WriteAllText(args[2],JsonSerializer.Serialize(cases));Console.WriteLine($"Wrote {cases.Count} original selection rectangles");return;
}
var createColor=colorType.GetMethod("FromBgra",[typeof(byte),typeof(byte),typeof(byte),typeof(byte)])!;
if (args.Length >= 4 && args[3] == "verify") {
    foreach (string file in Directory.EnumerateFiles(args[2], "*-rust.pdn")) {
        using var input=File.OpenRead(file);
        object doc=docType.GetMethod("FromStream",[typeof(Stream)])!.Invoke(null,[input])!;
        var all=new List<object>();
        foreach (object layer in (IList)Get(doc,"Layers")) {
            object surface=Get(layer,"Surface");
            var item=surface.GetType().GetProperty("Item",[typeof(int),typeof(int)])!;
            var pixels=new List<int[]>();
            int width=(int)Get(doc,"Width"),height=(int)Get(doc,"Height");
            for(int y=0;y<height;y++) for(int x=0;x<width;x++) {
                object color=item.GetValue(surface,[x,y])!;
                pixels.Add(new[]{"R","G","B","A"}.Select(n=>Convert.ToInt32(colorType.GetField(n)!.GetValue(color))).ToArray());
            }
            all.Add(new {name=Get(layer,"Name"),visible=Get(layer,"Visible"),opacity=Get(layer,"Opacity"),mode=Convert.ToInt32(Get(layer,"BlendMode")),pixels});
        }
        string json=JsonSerializer.Serialize(new{width=Get(doc,"Width"),height=Get(doc,"Height"),layers=all,dpu_x=Get(doc,"DpuX"),dpu_y=Get(doc,"DpuY")});
        File.WriteAllText(Path.ChangeExtension(file,"original-reader.json"),json);
        Console.WriteLine($"Original reader accepted {file}: {Get(doc,"Width")}x{Get(doc,"Height")}, {all.Count} layers, DPU {Get(doc,"DpuX")}x{Get(doc,"DpuY")}");
        (doc as IDisposable)?.Dispose();
    }
    return;
}
if (args.Length >= 4 && args[3] == "legacy") {
    using var input=File.OpenRead(Path.Combine(args[2],"small.pdn"));
    object doc=docType.GetMethod("FromStream",[typeof(Stream)])!.Invoke(null,[input])!;
    string file=Path.Combine(args[2],"small-legacy.pdn");
    using(var output=File.Create(file)) using(var gzip=new System.IO.Compression.GZipStream(output,System.IO.Compression.CompressionLevel.Optimal)) {
        new System.Runtime.Serialization.Formatters.Binary.BinaryFormatter().Serialize(gzip,doc);
    }
    Console.WriteLine($"Original serializer produced legacy gzip/inline-block fixture: {file}");
    (doc as IDisposable)?.Dispose();
    return;
}
Directory.CreateDirectory(args[2]);
foreach (var (name,width,height,layers) in new[]{("small",7,5,14),("chunks",257,257,2)}) {
    object doc=Activator.CreateInstance(docType,[width,height])!;
    Set(doc,"DpuX",144.0); Set(doc,"DpuY",120.0);
    var all=new List<object>();
    for (int n=0;n<layers;n++) {
        object layer=Activator.CreateInstance(layerType,[width,height])!;
        Set(layer,"Name",n==0 ? "Background α" : $"Layer {n} 日本語");
        Set(layer,"Visible",n%4!=3);
        Set(layer,"Opacity",(byte)(n==0?255:(n*17)%256));
        var blend=layerType.GetProperty("BlendMode")!;
        blend.SetValue(layer,Enum.ToObject(blend.PropertyType,n%14));
        object surface=Get(layer,"Surface");
        var item=surface.GetType().GetProperty("Item",[typeof(int),typeof(int)])!;
        var pixels=new List<int[]>();
        for (int y=0;y<height;y++) for (int x=0;x<width;x++) {
            byte r=(byte)((x*31+y*17+n*23)%256),g=(byte)((y*29+x*11+n*7)%256),b=(byte)((x+y*3+n*13)%256),a=(byte)((x*47+y*71+n*19)%256);
            object color=createColor.Invoke(null,[b,g,r,a])!;
            item.SetValue(surface,color,[x,y]);
            pixels.Add([r,g,b,a]);
        }
        ((IList)Get(doc,"Layers")).Add(layer);
        all.Add(new {name=Get(layer,"Name"), visible=Get(layer,"Visible"), opacity=Get(layer,"Opacity"),mode=Convert.ToInt32(Get(layer,"BlendMode")),pixels});
    }
    string output=Path.Combine(args[2],name+".pdn");
    using (var stream=File.Create(output)) docType.GetMethod("SaveToStream",[typeof(Stream)])!.Invoke(doc,[stream]);
    File.WriteAllText(Path.Combine(args[2],name+".json"),JsonSerializer.Serialize(new{width,height,layers=all,dpu_x=144.0,dpu_y=120.0}));
    Console.WriteLine($"Original writer produced {output}: {new FileInfo(output).Length} bytes");
    (doc as IDisposable)?.Dispose();
}

internal delegate T AverageColors<T>(ReadOnlySpan<T> colors) where T:struct;
internal static class PickerOracle {
    public static object Run<T>(MethodInfo method,Array colors) where T:struct => method.CreateDelegate<AverageColors<T>>()((T[])colors);
}
internal static class SelectionOracle {
    public static object DocumentValue=null!;
    public static T Document<T>(object tool) => (T)DocumentValue;
}
// Clone numerical method IL without changing operations or branch offsets.
// Only the Tool.Document getter is replaced, avoiding the unrelated WinForms
// workspace static constructor on Linux. All calls on the real Document remain.
internal static class IlOracle {
    public static DynamicMethod Clone(MethodInfo source,MethodInfo? replace=null,MethodInfo? substitute=null,Dictionary<FieldInfo,MethodInfo>? fields=null) {
        var body=source.GetMethodBody()!;byte[] code=body.GetILAsByteArray()!;
        var parameters=new[]{source.DeclaringType!}.Concat(source.GetParameters().Select(p=>p.ParameterType)).ToArray();
        var method=new DynamicMethod("oracle_"+source.Name,source.ReturnType,parameters,typeof(IlOracle).Module,true);
        var il=method.GetDynamicILInfo();
        var opcodes=typeof(OpCodes).GetFields(BindingFlags.Static|BindingFlags.Public).Where(f=>f.FieldType==typeof(OpCode)).Select(f=>(OpCode)f.GetValue(null)!).ToDictionary(o=>unchecked((ushort)o.Value));
        int token(MemberInfo m) => m switch {
            Type t => il.GetTokenFor(t.TypeHandle),
            MethodBase b => il.GetTokenFor(b.MethodHandle,b.DeclaringType!.TypeHandle),
            FieldInfo f => il.GetTokenFor(f.FieldHandle,f.DeclaringType!.TypeHandle),
            _=>throw new NotSupportedException(m.ToString()),
        };
        for(int p=0;p<code.Length;) {
            ushort value=code[p++];if(value==0xfe)value=(ushort)(0xfe00|code[p++]);var op=opcodes[value];
            if(op.OperandType is OperandType.InlineMethod or OperandType.InlineField or OperandType.InlineType or OperandType.InlineTok or OperandType.InlineString or OperandType.InlineSig) {
                int old=BitConverter.ToInt32(code,p);int updated;
                if(op.OperandType==OperandType.InlineString) updated=il.GetTokenFor(source.Module.ResolveString(old));
                else if(op.OperandType==OperandType.InlineSig) throw new NotSupportedException("Standalone signature in numerical oracle");
                else {
                    var member=source.Module.ResolveMember(old)!;
                    if(member is FieldInfo field && fields!=null && fields.TryGetValue(field,out var getter)) {
                        if(op!=OpCodes.Ldfld) throw new NotSupportedException("Only instance field reads may be substituted");
                        code[p-1]=unchecked((byte)OpCodes.Call.Value);updated=token(getter);
                    } else updated=token(member==replace?substitute!:member);
                }
                BitConverter.GetBytes(updated).CopyTo(code,p);p+=4;
            } else p+=op.OperandType switch {
                OperandType.InlineNone=>0,
                OperandType.ShortInlineI or OperandType.ShortInlineVar or OperandType.ShortInlineBrTarget=>1,
                OperandType.InlineVar=>2,
                OperandType.InlineI or OperandType.InlineBrTarget or OperandType.ShortInlineR=>4,
                OperandType.InlineI8 or OperandType.InlineR=>8,
                OperandType.InlineSwitch=>4+4*BitConverter.ToInt32(code,p),
                _=>throw new NotSupportedException(op.OperandType.ToString()),
            };
        }
        var signature=SignatureHelper.GetLocalVarSigHelper();foreach(var local in body.LocalVariables)signature.AddArgument(local.LocalType,local.IsPinned);
        il.SetLocalSignature(signature.GetSignature());
        if(body.ExceptionHandlingClauses.Count>0) {
            int size=4+24*body.ExceptionHandlingClauses.Count;byte[] exceptions=new byte[size];exceptions[0]=0x41;exceptions[1]=(byte)size;exceptions[2]=(byte)(size>>8);exceptions[3]=(byte)(size>>16);
            int offset=4;foreach(var c in body.ExceptionHandlingClauses) {
                int[] values=[(int)c.Flags,c.TryOffset,c.TryLength,c.HandlerOffset,c.HandlerLength,c.Flags==ExceptionHandlingClauseOptions.Clause?token(c.CatchType!):c.Flags==ExceptionHandlingClauseOptions.Filter?c.FilterOffset:0];
                foreach(int v in values) { BitConverter.GetBytes(v).CopyTo(exceptions,offset);offset+=4; }
            }
            il.SetExceptions(exceptions);
        }
        il.SetCode(code,body.MaxStackSize);return method;
    }
}


internal static class WheelOracle {
    public static bool SnapValue;
    public static DynamicMethod? ColorMethod;
    public static int Center(object? wheel)=>68;
    public static int Radius(object? wheel)=>68;
    public static bool Snap(object? wheel)=>SnapValue;
    public static T Color<T>(object? wheel,double h,double s,double a)=>(T)ColorMethod!.Invoke(null,[null,h,s,a])!;
}
