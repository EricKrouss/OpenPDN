use openpdn::{document::*, project};
use serde::Deserialize;
use std::{io::Cursor, sync::Arc};

#[derive(Deserialize)]
struct Fixture {
    mode: usize,
    dst: Pixel,
    src: Pixel,
    opacity: u8,
    expected: Pixel,
}

#[test]
fn original_paintnet_composition_fixtures() {
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-blends.json")).unwrap();
    for f in fixtures {
        assert_eq!(
            blend(f.dst, f.src, f.opacity, BlendMode::ALL[f.mode]),
            f.expected,
            "mode {:?}, dst {:?}, src {:?}, opacity {}",
            BlendMode::ALL[f.mode],
            f.dst,
            f.src,
            f.opacity
        );
    }
}
#[test]
fn exhaustive_original_blend_components() {
    let expected = include_bytes!("fixtures/paintnet-5.1.12-blend-components.bin");
    for (m, mode) in BlendMode::ALL.iter().enumerate() {
        for d in 0..256 {
            for s in 0..256 {
                assert_eq!(
                    blend_component(d, s, *mode) as u8,
                    expected[m * 65536 + d as usize * 256 + s as usize],
                    "{mode:?}: {d}, {s}"
                );
            }
        }
    }
}
#[test]
fn history_branch_saved_state_and_cow() {
    let doc = Document::new(8, 8, TRANSPARENT).unwrap();
    let original = doc.clone();
    let mut editor = Editor::new(doc);
    editor.edit("Brush", |d| {
        d.paint_line((2, 2), (4, 2), 1.0, [255, 0, 0, 255], false)
    });
    assert_ne!(editor.document, original);
    assert_eq!(original.layers[0].pixels[18], TRANSPARENT);
    assert!(editor.dirty());
    editor.mark_saved();
    editor.undo();
    assert!(editor.dirty());
    assert_eq!(editor.document, original);
    editor.redo();
    assert!(!editor.dirty());
    editor.undo();
    editor.edit("New Layer", Document::add_layer);
    assert!(!editor.can_redo());
    assert_eq!(editor.document.layers.len(), 2);
    assert_eq!(editor.document.active, 1);
}
#[test]
fn history_navigation_restores_mixed_edits_and_discards_abandoned_future() {
    let mut editor = Editor::new(Document::new(4, 3, TRANSPARENT).unwrap());
    let mut states = vec![editor.document.clone()];
    editor.edit("Paintbrush", |d| {
        d.paint_line((0, 0), (2, 0), 1.0, [20, 40, 60, 255], false)
    });
    states.push(editor.document.clone());
    editor.edit("Add New Layer", Document::add_layer);
    states.push(editor.document.clone());
    editor.mark_saved();
    editor.edit("Rectangle Select", |d| d.select_rect((1, 0), (2, 1), false));
    states.push(editor.document.clone());
    editor.edit("Rotate", Document::rotate_cw);
    states.push(editor.document.clone());
    for requested in [0, 4, 2, 3, 1, 0, usize::MAX] {
        editor.seek_history(requested);
        let position = requested.min(4);
        assert_eq!(editor.history_position(), position);
        assert_eq!(editor.document, states[position]);
    }
    editor.seek_history(2);
    assert!(!editor.dirty());
    editor.edit("Duplicate Layer", Document::duplicate_layer);
    let branch = editor.document.clone();
    editor.seek_history(usize::MAX);
    assert_eq!(editor.history_position(), 3);
    assert_eq!(editor.document, branch);
    assert!(!editor.can_redo());
    assert_eq!(editor.history().last().unwrap().0, "Duplicate Layer");
}
#[test]
fn selection_fill_erase_and_rotation() {
    let mut doc = Document::new(3, 2, [10, 20, 30, 255]).unwrap();
    doc.select_rect((1, 0), (1, 1), false);
    doc.fill((0, 0), [255; 4], 0); // Outside the mask: no mutation.
    assert!(doc.layers[0].pixels.iter().all(|p| *p == [10, 20, 30, 255]));
    doc.fill((1, 0), [255, 0, 0, 255], 0);
    assert_eq!(doc.layers[0].pixels[1], [255, 0, 0, 255]);
    assert_eq!(doc.layers[0].pixels[4], [255, 0, 0, 255]);
    let selected = doc.clone();
    for _ in 0..4 {
        doc.rotate_cw();
    }
    assert_eq!(doc, selected);
    doc.clear_selection_pixels();
    assert_eq!(doc.layers[0].pixels[1], TRANSPARENT);
    assert_eq!(doc.layers[0].pixels[0], [10, 20, 30, 255]);
    doc.validate().unwrap();
}
#[test]
fn native_project_roundtrip_and_atomic_overwrite() {
    let mut doc = Document::new(7, 4, [0; 4]).unwrap();
    doc.add_layer();
    doc.layers[1].opacity = 123;
    doc.layers[1].blend_mode = BlendMode::Reflect;
    doc.layers[1].visible = false;
    doc.layers[1].name = "日本語 layer".into();
    Arc::make_mut(&mut doc.layers[1].pixels)[6] = [70, 80, 90, 150];
    let mut bytes = Cursor::new(Vec::new());
    project::write_project(&doc, &mut bytes).unwrap();
    bytes.set_position(0);
    let loaded = project::read_project(bytes).unwrap();
    assert_eq!(doc.layers, loaded.layers);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("project.opdn");
    project::save_project(&doc, &path).unwrap();
    doc.layers[0].name = "Replaced".into();
    project::save_project(&doc, &path).unwrap();
    assert_eq!(project::open(&path).unwrap().layers, doc.layers);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn malformed_inputs_are_errors() {
    assert!(Document::new(0, 10, TRANSPARENT).is_err());
    assert!(Document::new(u32::MAX, u32::MAX, TRANSPARENT).is_err());
    assert!(project::read_project(Cursor::new(b"not a zip")).is_err());
    let mut doc = Document::new(1, 1, TRANSPARENT).unwrap();
    doc.layers[0].pixels = Arc::new(vec![]);
    assert!(doc.validate().is_err());
    assert!(project::write_project(&doc, Cursor::new(Vec::new())).is_err());
}
#[test]
fn full_erase_clears_rgb_and_single_stamp_preserves_alpha() {
    let mut doc = Document::new(1, 1, TRANSPARENT).unwrap();
    doc.paint_line((0, 0), (0, 0), 1.0, [90, 80, 70, 128], false);
    assert_eq!(doc.layers[0].pixels[0], [90, 80, 70, 128]);
    doc.paint_line((0, 0), (0, 0), 1.0, [0, 0, 0, 255], true);
    assert_eq!(doc.layers[0].pixels[0], TRANSPARENT);
}

#[test]
fn raster_exports_preserve_lossless_pixels_and_flatten_jpeg_alpha() {
    let dir = tempfile::tempdir().unwrap();
    let pixels = vec![
        [10, 20, 30, 0],
        [40, 50, 60, 128],
        [70, 80, 90, 255],
        [120, 130, 140, 1],
    ];
    for ext in ["png", "tiff", "webp", "qoi"] {
        let path = dir.path().join(format!("roundtrip.{ext}"));
        project::export(&path, 2, 2, &pixels).unwrap();
        let loaded = project::open(&path).unwrap();
        assert_eq!(loaded.layers[0].pixels.as_ref(), &pixels, "{ext}");
    }
    let path = dir.path().join("white.jpg");
    project::export(&path, 2, 2, &[[10, 20, 30, 0]; 4]).unwrap();
    let loaded = project::open(&path).unwrap();
    for p in loaded.layers[0].pixels.iter() {
        assert!(p[0] >= 250 && p[1] >= 250 && p[2] >= 250 && p[3] == 255);
    }
}

#[test]
fn version_one_development_projects_remain_readable() {
    use std::io::Write;
    let mut output = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut output);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(br#"{"format":"OpenDotNet","version":1,"width":1,"height":1,"layers":[{"name":"old layer","visible":false,"opacity":80,"blend_mode":"Multiply"}]}"#).unwrap();
    zip.start_file("layers/0.rgba", options).unwrap();
    zip.write_all(&[12, 34, 56, 78]).unwrap();
    zip.finish().unwrap();
    output.set_position(0);
    let loaded = project::read_project(output).unwrap();
    assert_eq!(loaded.layers[0].pixels.as_ref(), &vec![[12, 34, 56, 78]]);
    assert_eq!(loaded.layers[0].blend_mode, BlendMode::Multiply);
    assert_eq!(loaded.layers[0].opacity, 80);
    assert!(!loaded.layers[0].visible);
    assert!(loaded.metadata.is_empty());
    assert!(loaded.layers[0].metadata.is_empty());
}
