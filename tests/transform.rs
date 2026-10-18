use openpdn::{
    document::{BlendMode, Document, Editor, blend},
    editing::ClipboardImage,
    gpu::Gpu,
    transform::{FloatingSelection, PastePlan},
};
use std::sync::Arc;

fn source() -> Document {
    let mut doc = Document::new(7, 5, [0; 4]).unwrap();
    doc.layers[0].pixels = Arc::new(
        (0..35)
            .map(|i| [(i * 7) as u8, (i * 3) as u8, 50, 255])
            .collect(),
    );
    let mut mask = vec![0; 35];
    for y in 1..3 {
        for x in 2..5 {
            mask[y * 7 + x] = 255;
        }
    }
    doc.selection = Some(Arc::new(mask));
    doc
}
#[test]
fn floating_sources_keep_original_pixels_and_partial_coverage() {
    let doc = source();
    let f = FloatingSelection::from_selection(&doc, true, false)
        .unwrap()
        .unwrap();
    assert_eq!((f.width, f.height), (3, 2));
    assert_eq!(f.corners(), [[2., 1.], [5., 1.], [5., 3.], [2., 3.]]);
    assert_eq!(f.pixels[0], doc.layers[0].pixels[9]);
    assert_eq!(f.base.layers[0].pixels[9][3], 0);
    assert_eq!(f.before, doc);
    let f = FloatingSelection::from_selection(&doc, true, true)
        .unwrap()
        .unwrap();
    assert_eq!(f.base, doc);
    let mut doc = doc;
    Arc::make_mut(doc.selection.as_mut().unwrap())[9] = 128;
    let f = FloatingSelection::from_selection(&doc, true, false)
        .unwrap()
        .unwrap();
    assert_eq!(f.base.layers[0].pixels[9][3], 127);
    assert_eq!(f.mask[0], 128);
    let f = FloatingSelection::from_selection(&doc, false, false)
        .unwrap()
        .unwrap();
    assert_eq!(f.base, doc);
    doc.selection = Some(Arc::new(vec![0; 35]));
    assert!(
        FloatingSelection::from_selection(&doc, true, false)
            .unwrap()
            .is_none()
    );
}
#[test]
fn floating_paste_keeps_canvas_and_layers_reversible() {
    let doc = source();
    let image = ClipboardImage {
        width: 9,
        height: 6,
        pixels: vec![[30, 50, 90, 127]; 54],
        origin: [-5, 99],
    };
    let f = FloatingSelection::from_paste(&doc, image, true, true).unwrap();
    assert_eq!(f.before, doc);
    assert_eq!(
        (
            f.base.width,
            f.base.height,
            f.base.layers.len(),
            f.base.active
        ),
        (9, 6, 2, 1)
    );
    assert_eq!(f.corners(), [[0., 0.], [9., 0.], [9., 6.], [0., 6.]]);
    assert!(f.base.layers[1].pixels.iter().all(|p| *p == [0; 4]));
    let mut t = f.transform;
    for bad in [f32::NAN, f32::INFINITY, 0., 1001.] {
        t.scale[0] = bad;
        assert!(t.validate().is_err());
    }
    t = f.transform;
    t.center[0] = f32::INFINITY;
    assert!(t.validate().is_err());
}

#[test]
fn paste_preparations_follow_original_layer_canvas_and_pixel_undo_order() {
    let mut original = source();
    original
        .metadata
        .push(("opaque-profile".into(), "kept".into()));
    original.add_layer();
    original.layers[1].name = "Existing upper layer".into();
    original.layers[1].opacity = 73;
    original.layers[1].blend_mode = BlendMode::Multiply;
    original.active = 0;
    let image = ClipboardImage {
        width: 9,
        height: 3,
        pixels: vec![[20, 180, 90, 128]; 27],
        origin: [0, 0],
    };
    for new_layer in [false, true] {
        for expand in [false, true] {
            let mut editor = Editor::new(original.clone());
            let plan = PastePlan::new(&original, image.clone(), new_layer, expand).unwrap();
            assert_eq!(editor.document, original); // Prompt/cancellation hasn't committed anything.
            assert_eq!(editor.history_position(), 0);
            let f = plan.begin(&mut editor);
            let prepared = editor.document.clone();
            assert_eq!(f.before, prepared);
            assert_eq!(f.base, prepared);
            assert_eq!((f.width, f.height), (9, 3));
            assert_eq!(f.pixels.as_ref(), &image.pixels); // Keep retains clipped source pixels.
            assert_eq!(prepared.width, if expand { 9 } else { 7 });
            assert_eq!(prepared.height, 5); // Expansion must not shrink the other axis.
            assert_eq!(prepared.layers.len(), 2 + usize::from(new_layer));
            assert_eq!(prepared.active, usize::from(new_layer));
            assert_eq!(prepared.metadata, original.metadata);
            let upper = &prepared.layers[1 + usize::from(new_layer)];
            assert_eq!(upper.name, "Existing upper layer");
            assert_eq!(upper.opacity, 73);
            assert_eq!(upper.blend_mode, BlendMode::Multiply);
            let labels: Vec<_> = editor.history().map(|(label, _)| label).collect();
            let expected: Vec<_> = [(new_layer, "Add New Layer"), (expand, "Canvas Size")]
                .into_iter()
                .filter_map(|(present, label)| present.then_some(label))
                .collect();
            assert_eq!(labels, expected);
            let mut pasted = prepared.clone();
            pasted.paste_image(&image, [0, 0], false).unwrap();
            editor.document = pasted.clone();
            editor.record(f.label(), f.before);
            editor.undo();
            assert_eq!(editor.document, prepared); // Undo pixels before canvas/layer.
            if expand {
                editor.undo();
                assert_eq!((editor.document.width, editor.document.height), (7, 5));
                assert_eq!(editor.document.layers.len(), 2 + usize::from(new_layer));
            }
            if new_layer {
                editor.undo();
            }
            assert_eq!(editor.document, original);
            while editor.can_redo() {
                editor.redo();
            }
            assert_eq!(editor.document, pasted);
        }
    }
    // A cancelled plan, invalid image or layer-limit error leaves existing
    // history and its redo branch intact.
    let mut editor = Editor::new(original.clone());
    editor.edit("Edit", |doc| doc.layers[0].name = "edited".into());
    editor.undo();
    drop(PastePlan::new(&editor.document, image.clone(), true, true).unwrap());
    assert_eq!(editor.document, original);
    assert_eq!(editor.history_position(), 0);
    assert!(editor.can_redo());
    let mut invalid = image.clone();
    invalid.pixels.clear();
    assert!(PastePlan::new(&editor.document, invalid, true, true).is_err());
    assert_eq!(editor.document, original);
    assert!(editor.can_redo());
    let mut full = Document::new(4096, 1, [0; 4]).unwrap();
    while full.layers.len() < 256 {
        full.add_layer();
    }
    assert!(PastePlan::new(&full, image, true, false).is_err());
    let tall = ClipboardImage {
        width: 1,
        height: 4096,
        pixels: vec![[0; 4]; 4096],
        origin: [0, 0],
    };
    assert!(PastePlan::new(&full, tall, false, true).is_err());
    assert_eq!((full.width, full.height, full.layers.len()), (4096, 1, 256));
}
#[test]
#[ignore = "requires an actual hardware GPU"]
fn hardware_floating_transform_and_reversible_paste() {
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert!(matches!(
        gpu.info.device_type,
        eframe::egui_wgpu::wgpu::DeviceType::DiscreteGpu
            | eframe::egui_wgpu::wgpu::DeviceType::IntegratedGpu
    ));
    eprintln!(
        "Transform adapter: {} / {:?}",
        gpu.info.name, gpu.info.backend
    );
    let doc = source();
    let mut f = FloatingSelection::from_selection(&doc, true, false)
        .unwrap()
        .unwrap();
    assert_eq!(gpu.transform_selection(&f, false, false).unwrap(), doc);
    f.transform.center[0] += 1.;
    let moved = gpu.transform_selection(&f, false, false).unwrap();
    for y in 0..5 {
        for x in 0..7 {
            let i = y * 7 + x;
            let expected = if (1..3).contains(&y) && (3..6).contains(&x) {
                doc.layers[0].pixels[i - 1]
            } else if (1..3).contains(&y) && (2..5).contains(&x) {
                [
                    doc.layers[0].pixels[i][0],
                    doc.layers[0].pixels[i][1],
                    50,
                    0,
                ]
            } else {
                doc.layers[0].pixels[i]
            };
            assert_eq!(moved.layers[0].pixels[i], expected, "{x},{y}");
        }
    }
    // Always resample the retained source: moving out and back loses no pixels.
    f.transform.center = [-100., -100.];
    let clipped = gpu.transform_selection(&f, false, false).unwrap();
    assert!(clipped.selection.unwrap().iter().all(|m| *m == 0));
    f.transform = f.original_transform;
    assert_eq!(gpu.transform_selection(&f, true, true).unwrap(), doc);
    // Clockwise 90 degrees around the center of a 2x2 pasted tile.
    let blank = Document::new(4, 4, [0; 4]).unwrap();
    let tile = vec![
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
    ];
    let mut f = FloatingSelection::from_paste(
        &blank,
        ClipboardImage {
            width: 2,
            height: 2,
            pixels: tile.clone(),
            origin: [1, 1],
        },
        false,
        false,
    )
    .unwrap();
    f.transform.angle = 90.;
    let rotated = gpu.transform_selection(&f, false, false).unwrap();
    for (i, expected) in [(5, tile[2]), (6, tile[0]), (9, tile[3]), (10, tile[1])] {
        assert_eq!(rotated.layers[0].pixels[i], expected);
    }
    // Negative scale reflects; enlarged nearest pixels remain exact.
    f.transform.angle = 0.;
    f.transform.scale = [-2., 2.];
    let scaled = gpu.transform_selection(&f, false, false).unwrap();
    assert_eq!(scaled.layers[0].pixels[0], tile[1]);
    assert_eq!(scaled.layers[0].pixels[15], tile[2]);
    assert!(scaled.selection.as_ref().unwrap().iter().all(|m| *m == 255));
    // Linear light interpolation has the expected 188 midpoint, with alpha-correct colors.
    let mut f = FloatingSelection::from_paste(
        &blank,
        ClipboardImage {
            width: 2,
            height: 1,
            pixels: vec![[0, 0, 0, 255], [255; 4]],
            origin: [0, 1],
        },
        false,
        false,
    )
    .unwrap();
    f.transform.center[0] += 0.5;
    let gamma = gpu.transform_selection(&f, true, true).unwrap();
    let srgb = gpu.transform_selection(&f, true, false).unwrap();
    assert_eq!(gamma.layers[0].pixels[5], [188, 188, 188, 255]);
    assert_eq!(srgb.layers[0].pixels[5], [128, 128, 128, 255]);
    f.pixels = Arc::new(vec![[255, 0, 0, 0], [0, 0, 255, 255]]);
    let transparent = gpu.transform_selection(&f, true, false).unwrap();
    assert_eq!(transparent.layers[0].pixels[5], [0, 0, 255, 128]);
    // Mask-only transforms do not modify layer bytes.
    let mut f = FloatingSelection::from_selection(&doc, false, false)
        .unwrap()
        .unwrap();
    f.transform.center[1] += 1.;
    let mask_only = gpu.transform_selection(&f, false, false).unwrap();
    assert_eq!(mask_only.layers, doc.layers);
    assert_eq!(mask_only.selection.as_ref().unwrap()[2 * 7 + 2], 255);
    assert_eq!(mask_only.selection.as_ref().unwrap()[7 + 2], 0);
    // Paste uses straight-alpha source-over and is a single reversible transaction.
    let base = Document::new(3, 2, [90, 70, 30, 120]).unwrap();
    let image = ClipboardImage {
        width: 1,
        height: 1,
        pixels: vec![[20, 180, 90, 128]],
        origin: [1, 0],
    };
    let f = FloatingSelection::from_paste(&base, image, true, false).unwrap();
    let pasted = gpu.transform_selection(&f, false, false).unwrap();
    assert_eq!(
        pasted.composite()[1],
        blend(
            base.layers[0].pixels[1],
            [20, 180, 90, 128],
            255,
            BlendMode::Normal
        )
    );
    let mut editor = Editor::new(base.clone());
    editor.document = pasted.clone();
    editor.record("Paste", base.clone());
    editor.undo();
    assert_eq!(editor.document, base);
    editor.redo();
    assert_eq!(editor.document, pasted);
    let mut f = f;
    f.transform.scale[0] = 0.;
    assert!(gpu.transform_selection(&f, false, false).is_err());
}
