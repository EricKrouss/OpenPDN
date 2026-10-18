using System.Reflection;
using System.Runtime.Loader;
using System.Text.Json;

// Developer-only oracle. The Rust application never loads or ships .NET assemblies.
if (args.Length != 2) throw new ArgumentException("Usage: reference-probe EXTRACTED-X64 OUTPUT.json");
string folder = Path.GetFullPath(args[0]);
AssemblyLoadContext.Default.Resolving += (_, name) => {
    string path = Path.Combine(folder, name.Name + ".dll");
    return File.Exists(path) ? AssemblyLoadContext.Default.LoadFromAssemblyPath(path) : null;
};
var core = AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(folder, "PaintDotNet.Core.dll"));
var colorType = core.GetType("PaintDotNet.ColorBgra", true)!;
var create = colorType.GetMethod("FromBgra", [typeof(byte), typeof(byte), typeof(byte), typeof(byte)])!;
object Color(byte[] rgba) => create.Invoke(null, [rgba[2], rgba[1], rgba[0], rgba[3]])!;
byte[] Bytes(object value) => new[] { "R", "G", "B", "A" }.Select(n => (byte)colorType.GetField(n)!.GetValue(value)!).ToArray();
string[] modes = ["Normal", "Multiply", "Additive", "ColorBurn", "ColorDodge", "Reflect", "Glow", "Overlay", "Difference", "Negation", "Lighten", "Darken", "Screen", "Xor"];
var random = new Random(5112);
var normalType = core.GetType("PaintDotNet.Rendering.CompositionOps+Normal",true)!;
var normalApply = normalType.GetMethod("ApplyStatic",[colorType,colorType])!;
byte[] levels = [0, 1, 63, 127, 128, 191, 254, 255];
var fixtures = new List<object>();
for (int m=0; m<modes.Length; m++) {
    var type = core.GetType("PaintDotNet.Rendering.CompositionOps+" + modes[m], true)!;
    var createOp = type.GetMethod("Create", [typeof(int)])!;
    for (int n=0; n<192; n++) {
        byte[] dst = [levels[n%8], levels[n/8%8], (byte)random.Next(256), levels[n/24%8]];
        byte[] src = [levels[n/3%8], (byte)random.Next(256), levels[n/8%8], levels[n%8]];
        int opacity = levels[n/8%8];
        object op = createOp.Invoke(null, [opacity])!;
        var apply = op.GetType().GetMethod("Apply", [colorType, colorType])!;
        byte[] expected = Bytes(apply.Invoke(op, [Color(dst),Color(src)])!);
        object basePixel=normalApply.Invoke(null,[Color([0,0,0,0]),Color(dst)])!;
        byte[] composite=Bytes(apply.Invoke(op,[basePixel,Color(src)])!);
        fixtures.Add(new { mode=m, dst=dst.Select(b=>(int)b).ToArray(), src=src.Select(b=>(int)b).ToArray(), opacity,
            expected=expected.Select(b=>(int)b).ToArray(), composite=composite.Select(b=>(int)b).ToArray() });
    }
}
File.WriteAllText(args[1], JsonSerializer.Serialize(fixtures));
Console.WriteLine($"Wrote {fixtures.Count} cases from the original Paint.NET 5.1.12 composition routines.");
var table = new byte[14*256*256];
for (int m=0; m<modes.Length; m++) {
    var type = core.GetType("PaintDotNet.Rendering.CompositionOps+" + modes[m], true)!;
    var apply = type.GetMethod("ApplyStatic", [colorType,colorType])!;
    for (int d=0; d<256; d++) for (int s=0; s<256; s++) {
        table[m*65536+d*256+s] = Bytes(apply.Invoke(null,[Color([(byte)d,0,0,255]),Color([(byte)s,0,0,255])])!)[0];
    }
}
string tablePath = Path.Combine(Path.GetDirectoryName(args[1])!, "paintnet-5.1.12-blend-components.bin");
File.WriteAllBytes(tablePath,table);
Console.WriteLine($"Wrote {table.Length} exhaustive opaque channel cases from the original routines.");
