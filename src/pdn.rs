//! Paint.NET PDN3 codec, derived from the supplied 5.1.12 serialization routines.
//! NRBF is interpreted as inert data: no encoded .NET type is ever instantiated.
use crate::{
    document::{BlendMode, Document, Layer, Metadata, pixel_count},
    nrbf::{self, Graph, Value},
};
use anyhow::{Context, Result, bail, ensure};
use flate2::{Compression, bufread::GzDecoder, write::GzEncoder};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
    sync::Arc,
};
const LIMIT: usize = 512 * 1024 * 1024;
const VERSION: &str = "5.112.9563.32325";
const KVP: &str = "System.Collections.Generic.KeyValuePair`2[[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089],[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089]]";
const MODES: [&str; 14] = [
    "Normal",
    "Multiply",
    "Additive",
    "ColorBurn",
    "ColorDodge",
    "Reflect",
    "Glow",
    "Overlay",
    "Difference",
    "Negation",
    "Lighten",
    "Darken",
    "Screen",
    "Xor",
];

fn bounded_read(input: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    input.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit,
        "PDN data exceeds development memory limit"
    );
    Ok(bytes)
}
fn optional<'a>(g: &'a Graph, v: &'a Value, name: &str) -> Result<Option<&'a Value>> {
    g.resolve(v)?
        .fields()?
        .get(name)
        .map(|v| g.resolve(v))
        .transpose()
}
fn number(g: &Graph, v: &Value, name: &str) -> Result<usize> {
    usize::try_from(g.field(v, name)?.int()?).context("Negative or excessive PDN integer")
}
fn array(value: &Value) -> Result<&[Value]> {
    if let Value::Array(values) = value {
        Ok(values)
    } else {
        bail!("Expected PDN array")
    }
}
fn metadata(g: &Graph, value: &Value) -> Result<Metadata> {
    let Some(entries) = optional(g, value, "userMetadataItems")? else {
        ensure!(
            optional(g, value, "userMetaData")?.is_none(),
            "Legacy NameValueCollection metadata is not supported yet"
        );
        return Ok(vec![]);
    };
    array(entries)?
        .iter()
        .map(|item| {
            Ok((
                g.field(item, "key")?.string()?.to_owned(),
                g.field(item, "value")?.string()?.to_owned(),
            ))
        })
        .collect()
}
fn block_length(g: &Graph, v: &Value) -> Result<usize> {
    let n = optional(g, v, "length64")?
        .or(optional(g, v, "length")?)
        .context("Missing memory block length")?
        .int()?;
    let n = usize::try_from(n)?;
    ensure!(n > 0 && n <= LIMIT, "PDN memory block exceeds limit");
    Ok(n)
}
fn be32(input: &mut Cursor<&[u8]>) -> Result<usize> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes) as usize)
}
fn deferred(input: &mut Cursor<&[u8]>, length: usize) -> Result<Vec<u8>> {
    let mut format = [0];
    input.read_exact(&mut format)?;
    ensure!(format[0] <= 1, "Unknown PDN pixel compression");
    let size = be32(input)?;
    ensure!(size > 0, "PDN chunk size is zero");
    let count = length.div_ceil(size);
    ensure!(count <= 1_000_000, "Excessive PDN chunk count");
    let mut seen = vec![false; count];
    let mut pixels = vec![0; length];
    for _ in 0..count {
        let index = be32(input)?;
        ensure!(
            index < count && !seen[index],
            "Duplicate or out-of-range PDN chunk"
        );
        seen[index] = true;
        let stored = be32(input)?;
        let start = input.position() as usize;
        let end = start.checked_add(stored).context("Chunk length overflow")?;
        let bytes = input
            .get_ref()
            .get(start..end)
            .context("Truncated PDN chunk")?;
        let offset = index * size;
        let expected = size.min(length - offset);
        if format[0] == 0 {
            let mut decoder = GzDecoder::new(bytes);
            let decoded = bounded_read(&mut decoder, expected)?;
            ensure!(
                decoded.len() == expected && decoder.into_inner().is_empty(),
                "Incorrect compressed PDN chunk length"
            );
            pixels[offset..offset + expected].copy_from_slice(&decoded);
        } else {
            ensure!(bytes.len() == expected, "Incorrect raw PDN chunk length");
            pixels[offset..offset + expected].copy_from_slice(bytes);
        }
        input.set_position(end as u64);
    }
    Ok(pixels)
}
fn memory<'a>(
    g: &'a Graph,
    value: &'a Value,
    blocks: &'a BTreeMap<i32, Vec<u8>>,
    depth: usize,
) -> Result<&'a [u8]> {
    ensure!(depth <= 64, "Cyclic PDN parent memory block");
    let Value::Ref(id) = value else {
        bail!("Expected memory block reference")
    };
    let v = g.resolve(value)?;
    ensure!(
        v.class()? == "PaintDotNet.MemoryBlock",
        "Unexpected PDN memory class"
    );
    if g.field(v, "hasParent")?.bool()? {
        let parent = v
            .fields()?
            .get("parentBlock")
            .context("Missing parent memory block")?;
        let offset = optional(g, v, "parentOffset64")?
            .or(optional(g, v, "parentOffset")?)
            .context("Missing parent offset")?
            .int()?;
        let offset = usize::try_from(offset)?;
        let end = offset
            .checked_add(block_length(g, v)?)
            .context("Parent block range overflow")?;
        memory(g, parent, blocks, depth + 1)?
            .get(offset..end)
            .context("PDN child block exceeds parent")
    } else {
        Ok(blocks.get(id).context("Missing PDN pixel block")?)
    }
}

pub fn read(input: impl Read) -> Result<Document> {
    let bytes = bounded_read(input, LIMIT)?;
    let (header, body) = if bytes.starts_with(b"PDN") {
        ensure!(bytes.starts_with(b"PDN3"), "Unsupported PDN version");
        ensure!(bytes.len() >= 7, "Truncated PDN header");
        let length = bytes[4] as usize | ((bytes[5] as usize) << 8) | ((bytes[6] as usize) << 16);
        ensure!(length <= 1_048_576, "PDN XML header exceeds limit");
        let end = 7 + length;
        let xml = std::str::from_utf8(bytes.get(7..end).context("Truncated PDN XML header")?)?;
        ensure!(
            roxmltree::Document::parse(xml)?
                .root_element()
                .tag_name()
                .name()
                == "pdnImage",
            "Invalid PDN XML root"
        );
        (
            Some(xml.to_owned()),
            bytes.get(end..).context("Missing PDN body")?,
        )
    } else {
        (None, bytes.as_slice())
    };
    let inflated;
    let (body, is_deferred) = if body.starts_with(&[0, 1]) {
        (&body[2..], true)
    } else if body.starts_with(&[31, 139]) {
        inflated = bounded_read(GzDecoder::new(body), LIMIT)?;
        (inflated.as_slice(), false)
    } else {
        bail!("Invalid PDN serialization marker")
    };
    let g = nrbf::parse(body)?;
    let root = g.object(g.root)?;
    ensure!(
        root.class()? == "PaintDotNet.Document",
        "PDN root is not a document"
    );
    let width = u32::try_from(number(&g, root, "width")?)?;
    let height = u32::try_from(number(&g, root, "height")?)?;
    let count = pixel_count(width, height)?;
    let list = g.field(root, "layers")?;
    ensure!(
        list.class()? == "PaintDotNet.LayerList",
        "Unsupported PDN layer collection"
    );
    let size = number(&g, list, "ArrayList+_size")?;
    ensure!(
        size > 0 && size <= 256 && count * size * 4 <= LIMIT,
        "PDN layers exceed development memory limit"
    );
    let items = array(g.field(list, "ArrayList+_items")?)?;
    ensure!(size <= items.len(), "Truncated PDN layer collection");
    let mut blocks = BTreeMap::new();
    let mut input = Cursor::new(&body[g.end..]);
    let mut remaining = LIMIT;
    for id in &g.order {
        let v = g.objects.get(id).unwrap();
        if !matches!(v, Value::Object { class, .. } if class == "PaintDotNet.MemoryBlock")
            || g.field(v, "hasParent")?.bool()?
        {
            continue;
        }
        let length = block_length(&g, v)?;
        remaining = remaining
            .checked_sub(length)
            .context("PDN total pixel allocation exceeds limit")?;
        let data = if optional(&g, v, "deferred")?
            .map(Value::bool)
            .transpose()?
            .unwrap_or(false)
        {
            ensure!(is_deferred, "Deferred blocks inside legacy gzip graph");
            deferred(&mut input, length)?
        } else if let Some(Value::Bytes(b)) = optional(&g, v, "pointerData")? {
            ensure!(b.len() == length, "Invalid inline PDN memory length");
            b.clone()
        } else {
            ensure!(
                number(&g, v, "chunkFormat")? == 0,
                "Unsupported inline PDN chunks"
            );
            let n = number(&g, v, "chunkCount")?;
            ensure!(n <= 1_000_000, "Excessive inline PDN chunk count");
            let mut data = Vec::new();
            for i in 0..n {
                let Value::Bytes(b) = g.field(v, &format!("chunk{i}"))? else {
                    bail!("Invalid inline PDN chunk")
                };
                ensure!(
                    data.len() + b.len() <= length,
                    "Inline PDN chunks exceed block"
                );
                data.extend_from_slice(b);
            }
            ensure!(data.len() == length, "Truncated inline PDN pixels");
            data
        };
        blocks.insert(*id, data);
    }
    ensure!(
        input.position() as usize == input.get_ref().len(),
        "Unexpected trailing PDN pixel data"
    );
    let mut layers = Vec::new();
    for item in &items[..size] {
        let layer = g.resolve(item)?;
        ensure!(
            layer.class()? == "PaintDotNet.BitmapLayer",
            "Unsupported PDN layer type"
        );
        ensure!(
            number(&g, layer, "Layer+width")? == width as usize
                && number(&g, layer, "Layer+height")? == height as usize,
            "PDN layer dimensions differ"
        );
        let props = g.field(layer, "Layer+properties")?;
        let legacy = g.field(layer, "properties")?;
        let mut mode = optional(&g, props, "blendMode")?
            .map(|m| number(&g, m, "value__"))
            .transpose()?
            .unwrap_or(0);
        if let Some(op) = optional(&g, legacy, "blendOp")?.filter(|v| !matches!(v, Value::Null)) {
            mode = MODES
                .iter()
                .position(|m| {
                    op.class()
                        .is_ok_and(|c| c == format!("PaintDotNet.UserBlendOps+{m}BlendOp"))
                })
                .context("Unknown PDN blend operation")?;
        }
        let opacity = optional(&g, legacy, "opacity")?
            .or(optional(&g, props, "opacity")?)
            .map(Value::int)
            .transpose()?
            .unwrap_or(255);
        let surface = g.field(layer, "surface")?;
        ensure!(
            surface.class()? == "PaintDotNet.Surface"
                && number(&g, surface, "width")? == width as usize
                && number(&g, surface, "height")? == height as usize,
            "Invalid PDN surface dimensions"
        );
        let stride = number(&g, surface, "stride")?;
        ensure!(stride >= width as usize * 4, "PDN stride is too short");
        let data = memory(
            &g,
            surface
                .fields()?
                .get("scan0")
                .context("Missing surface pixels")?,
            &blocks,
            0,
        )?;
        let required = stride
            .checked_mul(height as usize)
            .context("Surface size overflow")?;
        ensure!(required <= data.len(), "Truncated PDN surface pixels");
        let mut pixels = Vec::with_capacity(count);
        for row in data[..required].chunks_exact(stride) {
            pixels.extend(
                row[..width as usize * 4]
                    .chunks_exact(4)
                    .map(|p| [p[2], p[1], p[0], p[3]]),
            );
        }
        layers.push(Layer {
            name: g.field(props, "name")?.string()?.into(),
            visible: g.field(props, "visible")?.bool()?,
            opacity: u8::try_from(opacity)?,
            blend_mode: BlendMode::ALL
                .get(mode)
                .copied()
                .with_context(|| format!("Unsupported PDN blend mode {mode}"))?,
            pixels: Arc::new(pixels),
            metadata: metadata(&g, props)?,
            is_background: g.field(props, "isBackground")?.bool()?,
        });
    }
    let mut saved_with = [0, 1, 0, 0];
    if let Some(version) = optional(&g, root, "savedWith")?.filter(|v| !matches!(v, Value::Null)) {
        for (out, name) in saved_with
            .iter_mut()
            .zip(["_Major", "_Minor", "_Build", "_Revision"])
        {
            *out = i32::try_from(g.field(version, name)?.int()?)?;
        }
    }
    let doc = Document {
        width,
        height,
        layers,
        active: 0,
        selection: None,
        metadata: metadata(&g, root)?,
        pdn_header: header,
        saved_with,
    };
    doc.validate()?;
    Ok(doc)
}

// Explicit wire types avoid BinaryFormatter's untrusted runtime type machinery.
#[derive(Clone)]
enum Wire {
    Bool(bool),
    Byte(u8),
    Int(i32),
    Long(i64),
    Ref(i32),
}
impl Wire {
    fn kind(&self) -> Option<u8> {
        match self {
            Self::Bool(_) => Some(1),
            Self::Byte(_) => Some(2),
            Self::Int(_) => Some(8),
            Self::Long(_) => Some(9),
            Self::Ref(_) => None,
        }
    }
    fn emit(&self, out: &mut Vec<u8>) {
        match self {
            Self::Bool(v) => out.push(u8::from(*v)),
            Self::Byte(v) => out.push(*v),
            Self::Int(v) => out.extend(v.to_le_bytes()),
            Self::Long(v) => out.extend(v.to_le_bytes()),
            Self::Ref(v) => {
                out.push(9);
                out.extend(v.to_le_bytes());
            }
        }
    }
}
fn text(out: &mut Vec<u8>, s: &str) {
    let mut n = s.len();
    while n >= 128 {
        out.push(n as u8 | 128);
        n >>= 7;
    }
    out.push(n as u8);
    out.extend(s.as_bytes());
}
struct Writer {
    next: i32,
    data: Vec<u8>,
    strings: Vec<(i32, String)>,
}
impl Writer {
    fn id(&mut self) -> i32 {
        let id = self.next;
        self.next += 1;
        id
    }
    fn string(&mut self, s: &str) -> Wire {
        let id = self.id();
        self.strings.push((id, s.into()));
        Wire::Ref(id)
    }
    fn class(&mut self, id: i32, name: &str, library: Option<i32>, fields: &[(&str, Wire)]) {
        self.data.push(if library.is_some() { 5 } else { 4 });
        self.data.extend(id.to_le_bytes());
        text(&mut self.data, name);
        self.data.extend((fields.len() as i32).to_le_bytes());
        for (name, _) in fields {
            text(&mut self.data, name);
        }
        for (_, value) in fields {
            self.data.push(if value.kind().is_some() { 0 } else { 2 });
        }
        for (_, value) in fields {
            if let Some(k) = value.kind() {
                self.data.push(k);
            }
        }
        if let Some(lib) = library {
            self.data.extend(lib.to_le_bytes());
        }
        for (_, value) in fields {
            value.emit(&mut self.data);
        }
    }
    fn metadata(&mut self, entries: &Metadata) -> i32 {
        let id = self.id();
        self.data.push(7);
        self.data.extend(id.to_le_bytes());
        self.data.push(0);
        self.data.extend(1i32.to_le_bytes());
        self.data.extend((entries.len() as i32).to_le_bytes());
        self.data.push(3);
        text(&mut self.data, KVP);
        for (key, value) in entries {
            let entry = -self.id();
            let key = self.string(key);
            let value = self.string(value);
            self.class(entry, KVP, None, &[("key", key), ("value", value)]);
        }
        id
    }
}
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
        .replace('\r', "&#13;")
        .replace('\n', "&#10;")
}
fn header(doc: &Document) -> Result<String> {
    let default = "<pdnImage><custom></custom></pdnImage>";
    let source = doc.pdn_header.as_deref().unwrap_or(default);
    ensure!(source.len() <= 1_048_576, "PDN XML header exceeds limit");
    let xml = roxmltree::Document::parse(source)?;
    let root = xml.root_element();
    ensure!(
        root.tag_name().name() == "pdnImage",
        "Invalid PDN header root"
    );
    let mut result = format!(
        "<pdnImage width=\"{}\" height=\"{}\" layers=\"{}\" savedWithVersion=\"{}.{}.{}.{}\"",
        doc.width,
        doc.height,
        doc.layers.len(),
        doc.saved_with[0],
        doc.saved_with[1],
        doc.saved_with[2],
        doc.saved_with[3]
    );
    for namespace in root.namespaces() {
        if namespace.name() == Some("xml") {
            continue;
        }
        let name = namespace
            .name()
            .map_or("xmlns".into(), |prefix| format!("xmlns:{prefix}"));
        result.push_str(&format!(" {name}=\"{}\"", xml_escape(namespace.uri())));
    }
    for attr in root
        .attributes()
        .filter(|a| !["width", "height", "layers", "savedWithVersion"].contains(&a.name()))
    {
        ensure!(
            attr.namespace().is_none(),
            "Namespaced PDN header attributes are not supported yet"
        );
        result.push_str(&format!(
            " {}=\"{}\"",
            attr.name(),
            xml_escape(attr.value())
        ));
    }
    result.push('>');
    for child in root.children() {
        result.push_str(&source[child.range()]);
    }
    result.push_str("</pdnImage>");
    Ok(result)
}

pub fn write(doc: &Document, mut output: impl Write) -> Result<()> {
    doc.validate()?;
    ensure!(
        doc.width as usize * doc.height as usize * doc.layers.len() * 4 <= LIMIT,
        "PDN pixels exceed development memory limit"
    );
    let header = header(doc)?;
    ensure!(header.len() <= 1_048_576, "PDN XML header exceeds limit");
    output.write_all(b"PDN3")?;
    output.write_all(&(header.len() as u32).to_le_bytes()[..3])?;
    output.write_all(header.as_bytes())?;
    output.write_all(&[0, 1])?;
    let mut w = Writer {
        next: 10,
        data: vec![0],
        strings: vec![],
    };
    for n in [1i32, -1, 1, 0] {
        w.data.extend(n.to_le_bytes());
    }
    for (id, name) in [(2i32, "PaintDotNet.Data"), (3, "PaintDotNet.Core")] {
        w.data.push(12);
        w.data.extend(id.to_le_bytes());
        text(
            &mut w.data,
            &format!("{name}, Version={VERSION}, Culture=neutral, PublicKeyToken=null"),
        );
    }
    let meta = w.id();
    w.class(
        1,
        "PaintDotNet.Document",
        Some(2),
        &[
            ("isDisposed", Wire::Bool(false)),
            ("layers", Wire::Ref(4)),
            ("width", Wire::Int(doc.width as i32)),
            ("height", Wire::Int(doc.height as i32)),
            ("savedWith", Wire::Ref(5)),
            ("userMetadataItems", Wire::Ref(meta)),
        ],
    );
    w.class(
        4,
        "PaintDotNet.LayerList",
        Some(2),
        &[
            ("parent", Wire::Ref(1)),
            ("ArrayList+_items", Wire::Ref(6)),
            ("ArrayList+_size", Wire::Int(doc.layers.len() as i32)),
            ("ArrayList+_version", Wire::Int(doc.layers.len() as i32)),
        ],
    );
    w.class(
        5,
        "System.Version",
        None,
        &[
            ("_Major", Wire::Int(doc.saved_with[0])),
            ("_Minor", Wire::Int(doc.saved_with[1])),
            ("_Build", Wire::Int(doc.saved_with[2])),
            ("_Revision", Wire::Int(doc.saved_with[3])),
        ],
    );
    // Reserve the metadata array ID used by the root; metadata() allocates it next.
    w.next -= 1;
    let actual = w.metadata(&doc.metadata);
    debug_assert_eq!(meta, actual);
    let layer_ids: Vec<_> = doc.layers.iter().map(|_| w.id()).collect();
    w.data.push(16);
    w.data.extend(6i32.to_le_bytes());
    w.data.extend((layer_ids.len() as i32).to_le_bytes());
    for id in &layer_ids {
        Wire::Ref(*id).emit(&mut w.data);
    }
    for (layer, id) in doc.layers.iter().zip(layer_ids) {
        let props = w.id();
        let bitmap = w.id();
        let surface = w.id();
        let memory = w.id();
        let mode = w.id();
        let op = w.id();
        w.class(
            id,
            "PaintDotNet.BitmapLayer",
            Some(2),
            &[
                ("properties", Wire::Ref(bitmap)),
                ("surface", Wire::Ref(surface)),
                ("Layer+isDisposed", Wire::Bool(false)),
                ("Layer+width", Wire::Int(doc.width as i32)),
                ("Layer+height", Wire::Int(doc.height as i32)),
                ("Layer+properties", Wire::Ref(props)),
            ],
        );
        let name = w.string(&layer.name);
        let meta = w.metadata(&layer.metadata);
        w.class(
            props,
            "PaintDotNet.Layer+LayerProperties",
            Some(2),
            &[
                ("name", name),
                ("userMetadataItems", Wire::Ref(meta)),
                ("visible", Wire::Bool(layer.visible)),
                ("isBackground", Wire::Bool(layer.is_background)),
                ("opacity", Wire::Byte(layer.opacity)),
                ("blendMode", Wire::Ref(mode)),
            ],
        );
        w.class(
            bitmap,
            "PaintDotNet.BitmapLayer+BitmapLayerProperties",
            Some(2),
            &[("blendOp", Wire::Ref(op))],
        );
        w.class(
            mode,
            "PaintDotNet.LayerBlendMode",
            Some(3),
            &[("value__", Wire::Int(layer.blend_mode as i32))],
        );
        w.class(
            op,
            &format!(
                "PaintDotNet.UserBlendOps+{}BlendOp",
                MODES[layer.blend_mode as usize]
            ),
            Some(2),
            &[],
        );
        w.class(
            surface,
            "PaintDotNet.Surface",
            Some(3),
            &[
                ("width", Wire::Int(doc.width as i32)),
                ("height", Wire::Int(doc.height as i32)),
                ("stride", Wire::Int(doc.width as i32 * 4)),
                ("scan0", Wire::Ref(memory)),
            ],
        );
        w.class(
            memory,
            "PaintDotNet.MemoryBlock",
            Some(3),
            &[
                (
                    "length64",
                    Wire::Long(doc.width as i64 * doc.height as i64 * 4),
                ),
                ("hasParent", Wire::Bool(false)),
                ("deferred", Wire::Bool(true)),
            ],
        );
    }
    for (id, s) in &w.strings {
        w.data.push(6);
        w.data.extend(id.to_le_bytes());
        text(&mut w.data, s);
    }
    w.data.push(11);
    output.write_all(&w.data)?;
    for layer in &doc.layers {
        output.write_all(&[0])?;
        output.write_all(&262144u32.to_be_bytes())?;
        for (i, chunk) in layer.pixels.chunks(65536).enumerate() {
            let mut compressed = GzEncoder::new(Vec::new(), Compression::default());
            let bgra: Vec<u8> = chunk
                .iter()
                .flat_map(|p| [p[2], p[1], p[0], p[3]])
                .collect();
            compressed.write_all(&bgra)?;
            let compressed = compressed.finish()?;
            output.write_all(&(i as u32).to_be_bytes())?;
            output.write_all(&(compressed.len() as u32).to_be_bytes())?;
            output.write_all(&compressed)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deferred_raw_out_of_order_duplicate_and_oversized_chunks() {
        let mut bytes = vec![1];
        bytes.extend(3u32.to_be_bytes());
        for (index, data) in [
            (1u32, b"def".as_slice()),
            (0, b"abc".as_slice()),
            (2, b"g".as_slice()),
        ] {
            bytes.extend(index.to_be_bytes());
            bytes.extend((data.len() as u32).to_be_bytes());
            bytes.extend(data);
        }
        assert_eq!(
            deferred(&mut Cursor::new(bytes.as_slice()), 7).unwrap(),
            b"abcdefg"
        );
        let mut duplicate = bytes.clone();
        duplicate[16..20].copy_from_slice(&1u32.to_be_bytes());
        assert!(deferred(&mut Cursor::new(duplicate.as_slice()), 7).is_err());
        let mut zero = bytes.clone();
        zero[1..5].fill(0);
        assert!(deferred(&mut Cursor::new(zero.as_slice()), 7).is_err());
        let mut compressed = GzEncoder::new(Vec::new(), Compression::default());
        compressed.write_all(&[0; 1024]).unwrap();
        let data = compressed.finish().unwrap();
        let mut bomb = vec![0];
        bomb.extend(3u32.to_be_bytes());
        bomb.extend(0u32.to_be_bytes());
        bomb.extend((data.len() as u32).to_be_bytes());
        bomb.extend(data);
        assert!(deferred(&mut Cursor::new(bomb.as_slice()), 3).is_err());
    }
}
