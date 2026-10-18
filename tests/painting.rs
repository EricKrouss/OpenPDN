use openpdn::{document::*, selection::AlphaMode};
use std::sync::Arc;

#[test]
fn clone_snapshots_opacity_selection_edges_and_undo() {
    let mut doc = Document::new(8, 2, [0, 0, 0, 255]).unwrap();
    Arc::make_mut(&mut doc.layers[0].pixels)[0] = [240, 120, 60, 255];
    let source = doc.layers[0].pixels.clone();
    let mut editor = Editor::new(doc);
    let before = editor.document.clone();
    editor.document.select_rect((2, 0), (5, 0), false);
    editor
        .document
        .clone_stroke(&source, (-2, 0), &[(2, 0), (5, 0)], 1., 128);
    assert_eq!(editor.document.layers[0].pixels[2], [120, 60, 30, 255]);
    assert_eq!(
        editor.document.layers[0].pixels[4],
        [0, 0, 0, 255],
        "Clone fed its newly written pixels back into the source"
    );
    assert_eq!(editor.document.layers[0].pixels[8], [0, 0, 0, 255]);
    editor.record("Clone Stamp", before.clone());
    editor.undo();
    assert_eq!(editor.document, before);
    editor.redo();
    assert_eq!(editor.document.layers[0].pixels[2], [120, 60, 30, 255]);
    let before = editor.document.clone();
    editor.document.clone_stroke(
        &source,
        (i32::MAX, i32::MIN),
        &[(i32::MIN, 0), (i32::MAX, 0)],
        1.,
        255,
    );
    assert_eq!(editor.document, before);
}

#[test]
fn recolor_shifts_channels_once_and_preserves_unmatched_pixels() {
    let mut doc = Document::new(5, 1, [100, 120, 140, 255]).unwrap();
    Arc::make_mut(&mut doc.layers[0].pixels)[4] = [20, 40, 60, 255];
    doc.recolor_stroke(
        &[(0, 0), (3, 0), (0, 0)],
        3.,
        [100, 120, 140, 255],
        [200, 80, 20, 255],
        0,
        AlphaMode::Straight,
    );
    assert_eq!(&doc.layers[0].pixels[..4], &[[200, 80, 20, 255]; 4]);
    assert_eq!(doc.layers[0].pixels[4], [20, 40, 60, 255]);
    let mut doc = Document::new(3, 1, [100, 120, 140, 255]).unwrap();
    doc.select_rect((1, 0), (1, 0), false);
    doc.recolor_stroke(
        &[(0, 0), (2, 0)],
        1.,
        [100, 120, 140, 255],
        [255, 255, 255, 255],
        0,
        AlphaMode::Premultiplied,
    );
    assert_eq!(
        &*doc.layers[0].pixels,
        &[[100, 120, 140, 255], [255; 4], [100, 120, 140, 255]]
    );
}
