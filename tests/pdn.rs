use openpdn::{document::*, pdn, project};
use serde::Deserialize;
use std::{io::Cursor, sync::Arc};
#[derive(Deserialize)]
struct Fixture {
    width: u32,
    height: u32,
    layers: Vec<ExpectedLayer>,
    dpu_x: f64,
    dpu_y: f64,
}
#[derive(Deserialize)]
struct ExpectedLayer {
    name: String,
    visible: bool,
    opacity: u8,
    mode: usize,
    pixels: Vec<Pixel>,
}
fn compare(doc: &Document, fixture: &str) {
    let expected: Fixture = serde_json::from_str(fixture).unwrap();
    assert_eq!((doc.width, doc.height), (expected.width, expected.height));
    let resolution = doc.resolution();
    assert_eq!(
        (resolution.x, resolution.y),
        (expected.dpu_x, expected.dpu_y)
    );
    assert_eq!(doc.layers.len(), expected.layers.len());
    for (actual, expected) in doc.layers.iter().zip(expected.layers) {
        assert_eq!(
            (
                &actual.name,
                actual.visible,
                actual.opacity,
                actual.blend_mode
            ),
            (
                &expected.name,
                expected.visible,
                expected.opacity,
                BlendMode::ALL[expected.mode]
            )
        );
        assert_eq!(actual.pixels.as_ref(), &expected.pixels);
    }
}
#[test]
fn original_pdn_layers_unicode_alpha_blends_and_multichunk_surfaces() {
    for (bytes, json) in [
        (
            include_bytes!("fixtures/pdn/small.pdn").as_slice(),
            include_str!("fixtures/pdn/small.json"),
        ),
        (
            include_bytes!("fixtures/pdn/chunks.pdn").as_slice(),
            include_str!("fixtures/pdn/chunks.json"),
        ),
    ] {
        let doc = pdn::read(bytes).unwrap();
        compare(&doc, json);
        assert_eq!(doc.saved_with, [5, 112, 9563, 32325]);
        assert_eq!(doc.metadata.len(), 3); // EXIF unit and 144x120 DPU.
        let mut output = Vec::new();
        pdn::write(&doc, &mut output).unwrap();
        let loaded = pdn::read(output.as_slice()).unwrap();
        compare(&loaded, json);
        assert_eq!(doc.metadata, loaded.metadata);
    }
}
#[test]
fn original_legacy_gzip_and_inline_block_serialization() {
    let doc = pdn::read(include_bytes!("fixtures/pdn/small-legacy.pdn").as_slice()).unwrap();
    compare(&doc, include_str!("fixtures/pdn/small.json"));
    assert!(doc.pdn_header.is_none());
    let current = pdn::read(include_bytes!("fixtures/pdn/small.pdn").as_slice()).unwrap();
    assert_eq!(doc.layers, current.layers);
    assert_eq!(doc.metadata, current.metadata);
}

#[test]
fn original_files_open_through_the_application_file_dispatch() {
    let directory = tempfile::tempdir().unwrap();
    for (name, expected) in [
        ("small.pdn", include_str!("fixtures/pdn/small.json")),
        ("chunks.pdn", include_str!("fixtures/pdn/chunks.json")),
        ("small-legacy.pdn", include_str!("fixtures/pdn/small.json")),
    ] {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdn")
            .join(name);
        let path = directory.path().join(name.to_uppercase());
        std::fs::copy(source, &path).unwrap();
        let doc = project::open(&path).unwrap();
        compare(&doc, expected);
        assert_eq!(doc.saved_with, [5, 112, 9563, 32325]);
        let saved = directory.path().join(format!("saved-{name}"));
        project::save_project(&doc, &saved).unwrap();
        let reloaded = project::open(&saved).unwrap();
        assert_eq!(doc.layers, reloaded.layers);
        assert_eq!(doc.metadata, reloaded.metadata);
        assert_eq!(doc.saved_with, reloaded.saved_with);
    }
}

#[test]
fn numeric_blends_without_legacy_operations_reject_unknown_modes() {
    // Mutate a COPY of the original serializer's fixture. Hiding the optional
    // legacy field exercises the numeric mode path used by newer documents.
    let mut bytes = include_bytes!("fixtures/pdn/small.pdn").to_vec();
    let field = b"\x07blendOp";
    let offsets: Vec<_> = bytes
        .windows(field.len())
        .enumerate()
        .filter_map(|(i, value)| (value == field).then_some(i))
        .collect();
    assert_eq!(offsets.len(), 14);
    for offset in offsets {
        bytes[offset + 1..offset + field.len()].copy_from_slice(b"oldMode");
    }
    compare(
        &pdn::read(bytes.as_slice()).unwrap(),
        include_str!("fixtures/pdn/small.json"),
    );
    // First enum record: member name, primitive type tags, library ID, Int32.
    let field = b"\x07value__\x00\x08";
    let offset = bytes.windows(field.len()).position(|v| v == field).unwrap() + field.len() + 4;
    for mode in [14i32, 255, i32::MAX, -1] {
        bytes[offset..offset + 4].copy_from_slice(&mode.to_le_bytes());
        assert!(
            pdn::read(bytes.as_slice()).is_err(),
            "Unsupported blend mode {mode} silently changed the document"
        );
    }
}

#[test]
fn unsupported_file_versions_layers_and_blend_operations_are_errors() {
    let original = include_bytes!("fixtures/pdn/small.pdn");
    for (from, to) in [
        (b"PDN3".as_slice(), b"PDN4".as_slice()),
        (b"PaintDotNet.BitmapLayer", b"PaintDotNet.VectorLayer"),
        (
            b"PaintDotNet.UserBlendOps+NormalBlendOp",
            b"PaintDotNet.UserBlendOps+FutureBlendOp",
        ),
    ] {
        assert_eq!(from.len(), to.len());
        let mut bytes = original.to_vec();
        let offsets: Vec<_> = bytes
            .windows(from.len())
            .enumerate()
            .filter_map(|(i, value)| (value == from).then_some(i))
            .collect();
        assert!(!offsets.is_empty());
        for offset in offsets {
            bytes[offset..offset + from.len()].copy_from_slice(to);
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("unsupported.pdn");
        std::fs::write(&path, &bytes).unwrap();
        assert!(project::open(&path).is_err(), "Accepted {to:?}");
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn metadata_background_header_atomic_save_and_development_format_preservation() {
    let mut doc = Document::new(5, 3, [80, 60, 40, 0]).unwrap();
    doc.metadata = vec![(
        "custom 日本語".into(),
        "opaque profile & resolution payload".into(),
    )];
    doc.layers[0].metadata = vec![("layer-key".into(), "layer-value".into())];
    doc.pdn_header = Some(
        "<pdnImage extra=\"a&amp;b\"><custom><entry>日本語</entry></custom></pdnImage>".into(),
    );
    doc.add_layer();
    doc.layers[1].blend_mode = BlendMode::Xor;
    Arc::make_mut(&mut doc.layers[1].pixels)[8] = [10, 20, 30, 99];
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("roundtrip.PDN");
    project::save_project(&doc, &path).unwrap();
    let loaded = project::open(&path).unwrap();
    assert_eq!(doc.layers, loaded.layers);
    assert_eq!(doc.metadata, loaded.metadata);
    assert!(
        loaded
            .pdn_header
            .as_ref()
            .unwrap()
            .contains("<entry>日本語</entry>")
    );
    assert!(
        loaded
            .pdn_header
            .as_ref()
            .unwrap()
            .contains("extra=\"a&amp;b\"")
    );
    let mut opdn = Cursor::new(Vec::new());
    project::write_project(&loaded, &mut opdn).unwrap();
    opdn.set_position(0);
    assert_eq!(project::read_project(opdn).unwrap(), loaded);
    project::save_project(&loaded, &path).unwrap();
    assert_eq!(project::open(&path).unwrap(), loaded);
    let saved_bytes = std::fs::read(&path).unwrap();
    let mut invalid = loaded.clone();
    invalid.pdn_header = Some("<broken".into());
    assert!(project::save_project(&invalid, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), saved_bytes);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}
#[test]
fn truncated_and_corrupt_pdn_are_errors() {
    let bytes = include_bytes!("fixtures/pdn/small.pdn");
    for length in [0, 1, 4, 6, 10, 100, bytes.len() - 1, bytes.len() / 2] {
        assert!(pdn::read(&bytes[..length]).is_err(), "length {length}");
    }
    let header = bytes[4] as usize | ((bytes[5] as usize) << 8) | ((bytes[6] as usize) << 16);
    let mut corrupt = bytes.to_vec();
    corrupt[7 + header] = 9;
    assert!(pdn::read(corrupt.as_slice()).is_err());
    let mut corrupt = bytes.to_vec();
    corrupt.extend([0, 1, 2]);
    assert!(pdn::read(corrupt.as_slice()).is_err());
}
