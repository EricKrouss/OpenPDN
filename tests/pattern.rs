use openpdn::{
    content::{ShapeMode, ShapeOptions},
    document::{Document, Pixel},
    line::{CurveMode, LineOptions},
    pattern::{FillSource, FillStyle, HatchStyle},
    selection::{AlphaMode, FloodMode},
    tool_blend::ToolBlend,
};
use serde::Deserialize;
#[derive(Deserialize)]
struct Case {
    style: usize,
    samples: Vec<Vec<Pixel>>,
}
#[test]
fn original_hatch_tiles_and_bounded_transparent_color_interpolation() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/native-hatches.json")).unwrap();
    assert_eq!(cases.len(), 53);
    let colors = [
        ([0, 0, 0, 255], [255; 4]),
        ([200, 40, 30, 128], [30, 40, 200, 255]),
        ([255, 0, 255, 0], [70, 120, 210, 255]),
        ([40, 210, 90, 255], [40, 80, 220, 128]),
    ];
    for case in cases {
        let style = HatchStyle::ALL[case.style];
        for (variant, (foreground, background)) in colors.into_iter().enumerate() {
            let source = FillSource {
                style: FillStyle::Hatch(style),
                foreground,
                background,
            };
            for (i, expected) in case.samples[variant].iter().enumerate() {
                let actual = source.sample((i % 8) as i32, (i / 8) as i32);
                let delta = actual
                    .iter()
                    .zip(expected)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                // Monochrome coverage is exact. GDI+ colored premultiplied
                // interpolation still differs by up to eight RGBA8 units.
                assert!(
                    delta <= if variant == 0 { 0 } else { 8 },
                    "{style:?}/{variant}/{i}: {actual:?} {expected:?}"
                );
                assert_eq!(
                    source.sample((i % 8) as i32 - 16, (i / 8) as i32 + 24),
                    actual
                );
            }
        }
    }
}
#[test]
fn patterned_brush_fill_lines_and_shapes_stay_image_anchored() {
    let source = FillSource {
        style: FillStyle::Hatch(HatchStyle::Horizontal),
        foreground: [0, 0, 0, 255],
        background: [255; 4],
    };
    let original = Document::new(24, 24, [80, 90, 100, 255]).unwrap();
    let mut brush = original.clone();
    brush.paint_stroke(&[(12, 12)], 20., source, false, ToolBlend::Normal);
    let mut fill = original.clone();
    fill.fill_blended(
        (0, 0),
        source,
        0,
        FloodMode::Global,
        AlphaMode::Premultiplied,
        false,
        ToolBlend::Normal,
    );
    let mut shape = original.clone();
    shape
        .draw_shape_at(
            (1., 1.),
            (23., 23.),
            source.foreground,
            source.background,
            &ShapeOptions {
                mode: ShapeMode::Fill,
                fill_style: source.style,
                antialias: false,
                ..Default::default()
            },
        )
        .unwrap();
    let mut line = original.clone();
    line.draw_curve(
        [[1., 12.], [8., 12.], [16., 12.], [23., 12.]],
        source,
        &LineOptions {
            mode: CurveMode::Straight,
            width: 20.,
            fill_style: source.style,
            antialias: false,
            ..Default::default()
        },
    )
    .unwrap();
    for doc in [brush, fill, shape, line] {
        for y in 7..=16 {
            for x in 8..=15 {
                assert_eq!(
                    doc.layers[0].pixels[y * 24 + x],
                    source.sample(x as i32, y as i32)
                );
            }
        }
    }
}
