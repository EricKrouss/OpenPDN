use openpdn::{
    content::{ShapeKind, ShapeMode, ShapeOptions},
    document::{BlendMode, Document, Editor, Pixel, blend},
    line::{CurveMode, LineOptions},
    selection::{AlphaMode, FloodMode},
    tool_blend::ToolBlend,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct OverwriteCase {
    dst: Pixel,
    src: Pixel,
    coverage: u8,
    result: Pixel,
}
#[test]
fn original_overwrite_masks_and_shared_composition_choices() {
    let cases: Vec<OverwriteCase> =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-overwrite.json")).unwrap();
    assert_eq!(cases.len(), 4096);
    for c in cases {
        assert_eq!(
            ToolBlend::Overwrite.apply(c.dst, c.src, c.coverage),
            c.result
        );
    }
    let modes: Vec<_> = ToolBlend::choices().collect();
    assert_eq!(modes.len(), 15);
    for (i, mode) in modes[..14].iter().enumerate() {
        for coverage in 0..=255 {
            assert_eq!(
                mode.apply([70, 180, 210, 128], [240, 90, 30, 169], coverage),
                blend(
                    [70, 180, 210, 128],
                    [240, 90, 30, 169],
                    coverage,
                    BlendMode::ALL[i]
                )
            );
        }
    }
}
#[test]
fn drawing_tools_share_blends_clip_masks_and_single_stroke_opacity() {
    let dst = [70, 180, 210, 128];
    let src = [255, 0, 255, 169];
    for mode in ToolBlend::choices() {
        let mut original = Document::new(15, 15, dst).unwrap();
        original.selection = Some(Arc::new(vec![128; 225]));
        let expected = mode.apply(dst, src, 128);
        let mut brush = original.clone();
        brush.paint_stroke(&[(6, 6), (8, 6), (6, 6), (7, 6)], 5., src, false, mode);
        assert_eq!(
            brush.layers[0].pixels[6 * 15 + 7],
            expected,
            "brush {mode:?}"
        );
        let mut bucket = original.clone();
        bucket.fill_blended(
            (0, 0),
            src,
            0,
            FloodMode::Contiguous,
            AlphaMode::Premultiplied,
            false,
            mode,
        );
        assert_eq!(bucket.layers[0].pixels[0], expected, "fill {mode:?}");
        let mut clone = original.clone();
        clone.clone_stroke_blended(&vec![src; 225], (0, 0), &[(7, 7)], 3., 255, mode);
        assert_eq!(
            clone.layers[0].pixels[7 * 15 + 7],
            expected,
            "clone {mode:?}"
        );
        let mut line = original.clone();
        line.draw_curve(
            [[3., 7.], [6., 7.], [9., 7.], [12., 7.]],
            src,
            &LineOptions {
                mode: CurveMode::Straight,
                width: 3.,
                antialias: false,
                overwrite: mode == ToolBlend::Overwrite,
                blend_mode: mode.mode(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(line.layers[0].pixels[7 * 15 + 7], expected, "line {mode:?}");
        let mut shape = original.clone();
        shape
            .draw_shape_at(
                (3., 3.),
                (12., 12.),
                src,
                [0; 4],
                &ShapeOptions {
                    kind: ShapeKind::Rectangle,
                    mode: ShapeMode::Fill,
                    antialias: false,
                    overwrite: mode == ToolBlend::Overwrite,
                    blend_mode: mode.mode(),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            shape.layers[0].pixels[7 * 15 + 7],
            expected,
            "shape {mode:?}"
        );
        let mut editor = Editor::new(original.clone());
        editor.document = brush;
        editor.record("Brush", original.clone());
        editor.undo();
        assert_eq!(editor.document, original);
        editor.redo();
    }
    let mut doc = Document::new(4, 4, [255; 4]).unwrap();
    doc.paint_stroke(
        &[(1, 1), (1, 1)],
        1.,
        [80, 90, 100, 0],
        false,
        ToolBlend::Overwrite,
    );
    assert_eq!(doc.layers[0].pixels[5], [80, 90, 100, 0]);
    doc.paint_stroke(
        &[(2, 2), (2, 2), (2, 2)],
        1.,
        [0, 0, 0, 128],
        true,
        ToolBlend::Normal,
    );
    assert_eq!(doc.layers[0].pixels[10], [255, 255, 255, 127]);
    let mut shape = Document::new(10, 10, [255; 4]).unwrap();
    shape
        .draw_shape_at(
            (2., 2.),
            (8., 8.),
            [0; 4],
            [0; 4],
            &ShapeOptions {
                mode: ShapeMode::Fill,
                overwrite: true,
                antialias: false,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(shape.layers[0].pixels[5 * 10 + 5], [0; 4]);
    assert_eq!(shape.layers[0].pixels[0], [255; 4]);
}
#[test]
#[ignore = "requires a hardware GPU"]
fn gradient_shared_blends_and_fractional_selection_match_cpu() {
    use openpdn::{gpu::Gpu, painting::GradientOptions};
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert_ne!(
        gpu.info.device_type,
        eframe::egui_wgpu::wgpu::DeviceType::Cpu
    );
    eprintln!(
        "Shared drawing blends GPU: {} / {:?}",
        gpu.info.name, gpu.info.backend
    );
    for mode in ToolBlend::choices() {
        for src in [[240, 90, 30, 169], [70, 80, 90, 0], [70, 80, 90, 255]] {
            let mut doc = Document::new(256, 1, [70, 180, 210, 128]).unwrap();
            doc.selection = Some(Arc::new((0..=255).collect()));
            let out = gpu
                .gradient(
                    &doc,
                    (0, 0),
                    (255, 0),
                    src,
                    src,
                    GradientOptions {
                        overwrite: mode == ToolBlend::Overwrite,
                        blend_mode: mode.mode(),
                        ..Default::default()
                    },
                )
                .unwrap();
            for (i, p) in out.iter().enumerate() {
                assert_eq!(
                    *p,
                    mode.apply(doc.layers[0].pixels[i], src, i as u8),
                    "{mode:?} {i} {src:?}"
                );
            }
        }
    }
}
