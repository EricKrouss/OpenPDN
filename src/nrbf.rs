//! Bounded, inert NRBF graph parser. Encoded type names are data, never executable.
//! Layout follows Microsoft's MS-NRBF specification; Paint.NET field interpretation
//! belongs to the PDN codec rather than this parser.
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Ref(i32),
    Int(i64),
    UInt(u64),
    Bool(bool),
    #[allow(dead_code)]
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Object {
        class: String,
        fields: BTreeMap<String, Value>,
    },
}
#[derive(Clone)]
enum FieldType {
    Primitive(u8),
    Other,
}
#[derive(Clone)]
struct Layout {
    class: String,
    names: Vec<String>,
    types: Vec<FieldType>,
}
pub struct Graph {
    pub root: i32,
    pub objects: BTreeMap<i32, Value>,
    pub end: usize,
    pub order: Vec<i32>,
}
impl Graph {
    pub fn resolve<'a>(&'a self, mut value: &'a Value) -> Result<&'a Value> {
        for _ in 0..64 {
            if let Value::Ref(id) = value {
                value = self
                    .objects
                    .get(id)
                    .with_context(|| format!("Missing NRBF object {id}"))?;
            } else {
                return Ok(value);
            }
        }
        bail!("Cyclic NRBF reference chain")
    }
    pub fn object(&self, id: i32) -> Result<&Value> {
        self.resolve(self.objects.get(&id).context("Missing NRBF object")?)
    }
    pub fn field<'a>(&'a self, value: &'a Value, name: &str) -> Result<&'a Value> {
        let Value::Object { fields, .. } = self.resolve(value)? else {
            bail!("Expected NRBF class");
        };
        self.resolve(
            fields
                .get(name)
                .with_context(|| format!("Missing NRBF member {name}"))?,
        )
    }
}
impl Value {
    pub fn int(&self) -> Result<i64> {
        match self {
            Self::Int(v) => Ok(*v),
            Self::UInt(v) => Ok(i64::try_from(*v)?),
            _ => bail!("Expected integer"),
        }
    }
    pub fn bool(&self) -> Result<bool> {
        if let Self::Bool(v) = self {
            Ok(*v)
        } else {
            bail!("Expected Boolean")
        }
    }
    pub fn string(&self) -> Result<&str> {
        if let Self::String(v) = self {
            Ok(v)
        } else {
            bail!("Expected string")
        }
    }
    pub fn fields(&self) -> Result<&BTreeMap<String, Value>> {
        if let Self::Object { fields, .. } = self {
            Ok(fields)
        } else {
            bail!("Expected object")
        }
    }
    pub fn class(&self) -> Result<&str> {
        if let Self::Object { class, .. } = self {
            Ok(class)
        } else {
            bail!("Expected object")
        }
    }
}
struct Parser<'a> {
    input: Cursor<&'a [u8]>,
    objects: BTreeMap<i32, Value>,
    layouts: BTreeMap<i32, Layout>,
    order: Vec<i32>,
    records: usize,
    budget: usize,
}
impl Parser<'_> {
    fn allocate(&mut self, bytes: usize) -> Result<()> {
        self.budget = self
            .budget
            .checked_sub(bytes)
            .context("NRBF graph exceeds memory budget")?;
        Ok(())
    }
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut b = [0; N];
        self.input.read_exact(&mut b)?;
        Ok(b)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.bytes::<1>()?[0])
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.bytes()?))
    }
    fn count(&mut self, max: usize) -> Result<usize> {
        let n = self.i32()?;
        ensure!(n >= 0 && n as usize <= max, "NRBF length exceeds limit");
        Ok(n as usize)
    }
    fn string(&mut self) -> Result<String> {
        let mut length = 0u32;
        for shift in (0..35).step_by(7) {
            let b = self.byte()?;
            ensure!(shift < 28 || b <= 15, "Invalid 7-bit string length");
            length |= u32::from(b & 127) << shift;
            if b & 128 == 0 {
                ensure!(length <= 16 * 1024 * 1024, "NRBF string exceeds limit");
                let available = self
                    .input
                    .get_ref()
                    .len()
                    .saturating_sub(self.input.position() as usize);
                ensure!(length as usize <= available, "Truncated NRBF string");
                self.allocate(length as usize * 4)?;
                let mut data = vec![0; length as usize];
                self.input.read_exact(&mut data)?;
                return Ok(String::from_utf8(data)?);
            }
        }
        bail!("Invalid NRBF string length")
    }
    fn primitive(&mut self, t: u8) -> Result<Value> {
        Ok(match t {
            1 => {
                let b = self.byte()?;
                ensure!(b <= 1, "Invalid Boolean");
                Value::Bool(b != 0)
            }
            2 => Value::UInt(self.byte()? as u64),
            3 => {
                // BinaryReader's UTF-8 Char is not a UTF-16 integer on the wire.
                let b = self.byte()?;
                let n = match b {
                    0..=127 => 1,
                    192..=223 => 2,
                    224..=239 => 3,
                    _ => bail!("Invalid NRBF char"),
                };
                let mut data = vec![b];
                for _ in 1..n {
                    data.push(self.byte()?);
                }
                Value::String(String::from_utf8(data)?)
            }
            5 | 18 => Value::String(self.string()?),
            6 => Value::Float(f64::from_le_bytes(self.bytes()?)),
            7 => Value::Int(i16::from_le_bytes(self.bytes()?) as i64),
            8 => Value::Int(self.i32()? as i64),
            9 | 12 => Value::Int(i64::from_le_bytes(self.bytes()?)),
            10 => Value::Int(self.byte()? as i8 as i64),
            11 => Value::Float(f32::from_le_bytes(self.bytes()?) as f64),
            13 | 16 => Value::UInt(u64::from_le_bytes(self.bytes()?)),
            14 => Value::UInt(u16::from_le_bytes(self.bytes()?) as u64),
            15 => Value::UInt(u32::from_le_bytes(self.bytes()?) as u64),
            17 => Value::Null,
            _ => bail!("Unsupported NRBF primitive {t}"),
        })
    }
    fn field_type(&mut self, t: u8) -> Result<FieldType> {
        Ok(match t {
            0 => FieldType::Primitive(self.byte()?),
            1 | 2 | 5 | 6 => FieldType::Other,
            3 => {
                self.string()?;
                FieldType::Other
            }
            4 => {
                self.string()?;
                self.i32()?;
                FieldType::Other
            }
            7 => {
                self.byte()?;
                FieldType::Other
            }
            _ => bail!("Unknown NRBF member type {t}"),
        })
    }
    fn store(&mut self, id: i32, value: Value) -> Result<Value> {
        ensure!(
            !self.objects.contains_key(&id),
            "Duplicate NRBF object ID {id}"
        );
        self.allocate(96)?;
        self.objects.insert(id, value);
        self.order.push(id);
        Ok(Value::Ref(id))
    }
    fn members(&mut self, id: i32, layout: &Layout, depth: usize) -> Result<Value> {
        self.allocate(
            layout.names.iter().map(|n| n.len()).sum::<usize>()
                + layout.class.len()
                + layout.names.len() * (std::mem::size_of::<Value>() + 80),
        )?;
        let mut fields = BTreeMap::new();
        for (name, t) in layout.names.iter().zip(&layout.types) {
            let value = match t {
                FieldType::Primitive(p) => self.primitive(*p)?,
                FieldType::Other => self.value(depth + 1)?,
            };
            fields.insert(name.clone(), value);
        }
        self.store(
            id,
            Value::Object {
                class: layout.class.clone(),
                fields,
            },
        )
    }
    fn array(&mut self, id: i32, n: usize, t: FieldType, depth: usize) -> Result<Value> {
        if matches!(t, FieldType::Primitive(2)) {
            ensure!(n <= 512 * 1024 * 1024, "Byte array exceeds limit");
            ensure!(
                n <= self
                    .input
                    .get_ref()
                    .len()
                    .saturating_sub(self.input.position() as usize),
                "Truncated NRBF byte array"
            );
            self.allocate(n)?;
            let mut bytes = vec![0; n];
            self.input.read_exact(&mut bytes)?;
            return self.store(id, Value::Bytes(bytes));
        }
        ensure!(n <= 1_000_000, "NRBF object array exceeds limit");
        self.allocate(n * std::mem::size_of::<Value>())?;
        let mut values = Vec::new();
        while values.len() < n {
            if let FieldType::Primitive(p) = t {
                values.push(self.primitive(p)?);
                continue;
            }
            let tag = self.byte()?;
            if tag == 13 || tag == 14 {
                let count = if tag == 13 {
                    self.byte()? as usize
                } else {
                    self.count(n)?
                };
                ensure!(
                    count > 0 && values.len() + count <= n,
                    "Invalid null array run"
                );
                values.resize(values.len() + count, Value::Null);
            } else {
                values.push(self.record(tag, depth + 1)?);
            }
        }
        self.store(id, Value::Array(values))
    }
    fn value(&mut self, depth: usize) -> Result<Value> {
        let tag = self.byte()?;
        self.record(tag, depth)
    }
    fn record(&mut self, mut tag: u8, depth: usize) -> Result<Value> {
        ensure!(depth <= 64, "NRBF nesting exceeds limit");
        loop {
            self.records += 1;
            ensure!(self.records <= 1_000_000, "NRBF record limit exceeded");
            if tag != 12 {
                break;
            }
            self.i32()?;
            self.string()?;
            tag = self.byte()?;
        }
        match tag {
            1 => {
                let id = self.i32()?;
                let metadata = self.i32()?;
                let layout = self
                    .layouts
                    .get(&metadata)
                    .context("Unknown NRBF class layout")?
                    .clone();
                self.members(id, &layout, depth)
            }
            4 | 5 => {
                let id = self.i32()?;
                let class = self.string()?;
                let count = self.count(4096)?;
                let names = (0..count)
                    .map(|_| self.string())
                    .collect::<Result<Vec<_>>>()?;
                ensure!(
                    names
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == names.len(),
                    "Duplicate NRBF field"
                );
                let tags = (0..count)
                    .map(|_| self.byte())
                    .collect::<Result<Vec<_>>>()?;
                let types = tags
                    .into_iter()
                    .map(|t| self.field_type(t))
                    .collect::<Result<Vec<_>>>()?;
                if tag == 5 {
                    self.i32()?;
                }
                let layout = Layout {
                    class,
                    names,
                    types,
                };
                ensure!(
                    !self.layouts.contains_key(&id),
                    "Duplicate NRBF metadata ID"
                );
                self.layouts.insert(id, layout.clone());
                self.members(id, &layout, depth)
            }
            6 => {
                let id = self.i32()?;
                let text = self.string()?;
                self.store(id, Value::String(text))
            }
            7 => {
                let id = self.i32()?;
                let kind = self.byte()?;
                ensure!(kind <= 5, "Invalid NRBF array type");
                let rank = self.count(32)?;
                ensure!(rank > 0, "Invalid array rank");
                let mut count = 1usize;
                for _ in 0..rank {
                    count = count
                        .checked_mul(self.count(512 * 1024 * 1024)?)
                        .context("Array length overflow")?;
                }
                if kind >= 3 {
                    for _ in 0..rank {
                        self.i32()?;
                    }
                }
                let kind = self.byte()?;
                let field_type = self.field_type(kind)?;
                self.array(id, count, field_type, depth)
            }
            8 => {
                let kind = self.byte()?;
                self.primitive(kind)
            }
            9 => Ok(Value::Ref(self.i32()?)),
            10 => Ok(Value::Null),
            15 => {
                let id = self.i32()?;
                let n = self.count(512 * 1024 * 1024)?;
                let t = self.byte()?;
                self.array(id, n, FieldType::Primitive(t), depth)
            }
            16 | 17 => {
                let id = self.i32()?;
                let n = self.count(1_000_000)?;
                self.array(id, n, FieldType::Other, depth)
            }
            _ => bail!("Unsupported NRBF record {tag}"),
        }
    }
}
pub fn parse(data: &[u8]) -> Result<Graph> {
    let mut parser = Parser {
        input: Cursor::new(data),
        objects: BTreeMap::new(),
        layouts: BTreeMap::new(),
        order: vec![],
        records: 0,
        budget: 512 * 1024 * 1024,
    };
    ensure!(parser.byte()? == 0, "Missing NRBF stream header");
    let root = parser.i32()?;
    parser.i32()?;
    ensure!(
        parser.i32()? == 1 && parser.i32()? == 0,
        "Unsupported NRBF stream version"
    );
    loop {
        let tag = parser.byte()?;
        if tag == 11 {
            break;
        }
        parser.record(tag, 0)?;
    }
    ensure!(
        parser.objects.contains_key(&root),
        "Missing NRBF root object"
    );
    Ok(Graph {
        root,
        objects: parser.objects,
        end: parser.input.position() as usize,
        order: parser.order,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_runs_are_iterative_and_truncated_arrays_do_not_allocate() {
        let mut bytes = vec![0];
        for n in [1i32, -1, 1, 0] {
            bytes.extend(n.to_le_bytes());
        }
        for _ in 0..100_000 {
            bytes.push(12);
            bytes.extend(2i32.to_le_bytes());
            bytes.extend([1, b'X']);
        }
        bytes.push(6);
        bytes.extend(1i32.to_le_bytes());
        bytes.extend([1, b'Y', 11]);
        assert_eq!(
            parse(&bytes).unwrap().object(1).unwrap().string().unwrap(),
            "Y"
        );
        bytes.truncate(17);
        bytes.push(15);
        bytes.extend(1i32.to_le_bytes());
        bytes.extend((512i32 * 1024 * 1024).to_le_bytes());
        bytes.push(2);
        assert!(parse(&bytes).is_err());
    }
    #[test]
    fn original_graph_is_inert_and_has_expected_root() {
        let data = include_bytes!("../tests/fixtures/pdn/small.pdn");
        let header = data[4] as usize | ((data[5] as usize) << 8) | ((data[6] as usize) << 16);
        let graph = parse(&data[9 + header..]).unwrap();
        assert_eq!(
            graph.object(graph.root).unwrap().class().unwrap(),
            "PaintDotNet.Document"
        );
        assert_eq!(
            graph
                .field(graph.object(graph.root).unwrap(), "width")
                .unwrap()
                .int()
                .unwrap(),
            7
        );
        assert!(graph.end > 0);
    }
}
