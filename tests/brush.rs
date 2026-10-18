use openpdn::{
    brush::{
        BrushOptions, BrushPoint, BrushShape, BrushTip, aliased_bounds, flow_opacity,
        pressure_scale, samples, smooth_point, soft_diameter,
    },
    document::{Document, Editor},
    tool_blend::ToolBlend,
};
use serde::Deserialize;
use std::sync::Arc;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Plan {
    size: f64,
    hardness: f64,
    flow: f64,
    pressure: f64,
    soft_diameter: f64,
    opacity: f64,
    scale: f64,
}
#[derive(Deserialize)]
struct Curve {
    input: [[f64; 3]; 4],
    t: f32,
    result: [f64; 3],
}
#[derive(Deserialize)]
struct Oracle {
    plans: Vec<Plan>,
    curves: Vec<Curve>,
    aliased: Vec<Alias>,
}
#[derive(Deserialize)]
struct Alias {
    size: f64,
    scale: f64,
    spacing: f64,
    center: [f64; 2],
    bounds: [f64; 4],
    pixels: f64,
}
fn point(p: [f64; 3]) -> BrushPoint {
    BrushPoint::new(p[0], p[1], p[2])
}
#[test]
fn original_brush_plans_and_pressure_aware_smoothing() {
    let oracle: Oracle =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-brush.json")).unwrap();
    assert_eq!(oracle.plans.len(), 256);
    assert_eq!(oracle.curves.len(), 128);
    assert_eq!(oracle.aliased.len(), 256);
    for a in oracle.aliased {
        assert_eq!(aliased_bounds(a.center, a.size, a.scale), a.bounds);
        assert_eq!(
            BrushOptions {
                antialiased: false,
                spacing: a.spacing,
                ..Default::default()
            }
            .spacing_pixels(a.size, a.scale),
            a.pixels
        );
    }
    for p in oracle.plans {
        assert_eq!(soft_diameter(p.size, p.hardness), p.soft_diameter);
        assert_eq!(flow_opacity(p.flow), p.opacity);
        assert_eq!(pressure_scale(p.pressure), p.scale);
    }
    for c in oracle.curves {
        let p = smooth_point(c.input.map(point), c.t);
        for (got, want) in [p.x, p.y, p.pressure].into_iter().zip(c.result) {
            assert!((got - want).abs() < 1e-10, "{got} vs {want}");
        }
    }
}
#[test]
fn stamp_phase_survives_event_boundaries_clipping_and_pressure() {
    let options = BrushOptions {
        spacing: 0.5,
        smoothing: false,
        ..Default::default()
    };
    let a = point([0.5, 0.5, 1.]);
    let b = point([30.5, 0.5, 1.]);
    let bounds = [-10., -10., 50., 50.];
    let whole = samples(&[a, b], 10., options, bounds);
    let segmented = samples(
        &[a, point([3.5, 0.5, 1.]), point([8.5, 0.5, 1.]), b],
        10.,
        options,
        bounds,
    );
    assert_eq!(whole.len(), 7);
    assert_eq!(whole.len(), segmented.len());
    for (a, b) in whole.iter().zip(&segmented) {
        assert!((a.x - b.x).abs() < 1e-6);
    }
    let huge = samples(
        &[point([-1e12, 0.5, 1.]), point([1e12, 0.5, 1.])],
        10.,
        options,
        bounds,
    );
    assert!(huge.len() <= 13);
    let varying = samples(
        &[point([0., 0., 0.]), point([50., 0., 1.])],
        10.,
        options,
        bounds,
    );
    assert!(varying.len() > whole.len());
    assert!(
        varying[1].x - varying[0].x < varying[varying.len() - 1].x - varying[varying.len() - 2].x
    );
    let aliased = BrushOptions {
        antialiased: false,
        spacing: 0.15,
        ..options
    };
    assert_eq!(aliased.spacing_pixels(11., 1.), 1.5);
    assert_eq!(aliased.spacing_pixels(1., pressure_scale(0.)), 1.);
    let diagonal = [point([0., 0., 1.]), point([10., 10., 1.])];
    assert_eq!(samples(&diagonal, 1., aliased, bounds).len(), 11);
    assert_eq!(samples(&diagonal, 1., options, bounds).len(), 15);
}
#[test]
fn tips_feather_space_change_shape_and_commit_once() {
    let options = BrushOptions {
        hardness: 1.,
        smoothing: false,
        spacing: 0.15,
        ..Default::default()
    };
    let center = point([32.5, 32.5, 1.]);
    let hard = BrushTip::new(20., options).mask(65, 65, &[center]);
    let soft_tip = BrushTip::new(
        20.,
        BrushOptions {
            hardness: 0.,
            ..options
        },
    );
    let soft = soft_tip.mask(65, 65, &[center]);
    assert!(hard[32 * 65 + 32] > 250);
    assert!(soft[32 * 65 + 32] < hard[32 * 65 + 32]);
    assert!(soft[32 * 65 + 43] > hard[32 * 65 + 43]);
    assert!(
        soft.iter().filter(|&&p| p > 0 && p < 255).count()
            > hard.iter().filter(|&&p| p > 0 && p < 255).count()
    );
    let square = BrushTip::new(
        20.,
        BrushOptions {
            shape: BrushShape::Square,
            ..options
        },
    )
    .mask(65, 65, &[center]);
    assert!(square[40 * 65 + 40] > hard[40 * 65 + 40]);
    let alias = BrushTip::new(
        20.,
        BrushOptions {
            antialiased: false,
            ..options
        },
    )
    .mask(65, 65, &[center]);
    assert!(alias.iter().all(|&p| p == 0 || p == 255));
    let low = BrushTip::new(20., options).mask(65, 65, &[point([32.5, 32.5, 0.1])]);
    assert!(low.iter().filter(|&&p| p != 0).count() < hard.iter().filter(|&&p| p != 0).count());
    let sparse = BrushTip::new(
        4.,
        BrushOptions {
            spacing: 5.,
            antialiased: false,
            ..options
        },
    );
    let line = [point([2.5, 5.5, 1.]), point([62.5, 5.5, 1.])];
    let mask = sparse.mask(65, 12, &line);
    assert_eq!(mask[5 * 65 + 12], 0);
    assert_eq!(mask[5 * 65 + 22], 255);
    let mut doc = Document::new(65, 65, [80, 100, 120, 255]).unwrap();
    doc.selection = Some(Arc::new(vec![128; 65 * 65]));
    let mut editor = Editor::new(doc);
    let before = editor.document.clone();
    let src = [200, 50, 40, 128];
    editor.document.paint_sampled_stroke(
        &[center, point([35.5, 32.5, 1.]), center],
        &BrushTip::new(20., options),
        src,
        false,
        ToolBlend::Normal,
    );
    assert_eq!(
        editor.document.layers[0].pixels[32 * 65 + 32],
        ToolBlend::Normal.apply([80, 100, 120, 255], src, 128)
    );
    editor.record("Paintbrush", before.clone());
    editor.undo();
    assert_eq!(editor.document, before);
    editor.redo();
    assert_ne!(editor.document, before);
    editor.document = before.clone();
    editor.document.paint_sampled_stroke(
        &[center],
        &soft_tip,
        [0, 0, 0, 255],
        true,
        ToolBlend::Overwrite,
    );
    assert!(editor.document.layers[0].pixels[32 * 65 + 32][3] < 255);
    assert_eq!(
        editor.document.layers[0].pixels[0],
        before.layers[0].pixels[0]
    );
    assert!(
        BrushOptions {
            hardness: f64::NAN,
            spacing: f64::INFINITY,
            ..options
        }
        .sanitized()
        .hardness
        .is_finite()
    );
    let large = BrushTip::new(
        2000.,
        BrushOptions {
            hardness: 0.,
            ..options
        },
    );
    assert!(large.mask(1, 1, &[point([0.5, 0.5, 1.])])[0] > 0);
}

#[test]
fn clone_and_recolor_use_soft_masks_and_fractional_selections() {
    use openpdn::{
        document::{Pixel, scale},
        selection::AlphaMode,
    };
    let tip = BrushTip::new(
        20.,
        BrushOptions {
            hardness: 0.5,
            smoothing: false,
            ..Default::default()
        },
    );
    let points = [point([25.5, 25.5, 1.]), point([35.5, 25.5, 1.])];
    let mask = tip.mask(65, 65, &points);
    let dst = [80, 100, 120, 255];
    let mut original = Document::new(65, 65, dst).unwrap();
    original.selection = Some(Arc::new(vec![128; 65 * 65]));
    let source: Vec<Pixel> = (0..65 * 65)
        .map(|i| [(i % 65) as u8, (i / 65) as u8, 200, 130])
        .collect();
    for mode in ToolBlend::choices() {
        let mut doc = original.clone();
        doc.clone_sampled_stroke(&source, (-1, 0), &points, &tip, 128, mode);
        for y in 0..65 {
            for x in 1..65 {
                let i = y * 65 + x;
                let coverage = scale(scale(u32::from(mask[i]), 128), 128) as u8;
                assert_eq!(
                    doc.layers[0].pixels[i],
                    mode.apply(dst, source[i - 1], coverage),
                    "Clone {mode:?} pixel {i}"
                );
            }
        }
    }
    let mut doc = original.clone();
    doc.recolor_sampled_stroke(
        &points,
        &tip,
        dst,
        [200, 50, 0, 255],
        0,
        AlphaMode::Straight,
    );
    for (i, &m) in mask.iter().enumerate() {
        assert_eq!(
            doc.layers[0].pixels[i],
            ToolBlend::Normal.apply(dst, [200, 50, 0, 255], scale(u32::from(m), 128) as u8)
        );
    }
    let unchanged = doc.clone();
    doc.recolor_sampled_stroke(
        &points,
        &tip,
        [1, 2, 3, 255],
        [255; 4],
        0,
        AlphaMode::Straight,
    );
    assert_eq!(doc, unchanged, "Unmatched Recolor pixels changed");
}
