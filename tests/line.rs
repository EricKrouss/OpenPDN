use openpdn::{
    document::{Document, Editor},
    line::*,
};
use std::sync::Arc;
#[test]
fn constrained_and_centered_lines_match_original_geometry() {
    #[derive(serde::Deserialize)]
    struct Case {
        start: [f64; 2],
        end: [f64; 2],
        endpoint: [f64; 2],
    }
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "fixtures/paintnet-5.1.12-line-constraints.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 256);
    for (index, case) in cases.iter().enumerate() {
        let actual = constrain_endpoint(case.start, case.end);
        for (a, b) in actual.into_iter().zip(case.endpoint) {
            assert!(
                (a - b).abs() < 1e-9,
                "case {index}: {actual:?} vs {:?}",
                case.endpoint
            );
        }
    }
    let regular = initial_points_with_modifiers((100, 100), (200, 127), true, false);
    let centered = initial_points_with_modifiers((100, 100), (200, 127), true, true);
    assert_eq!(regular[3], centered[3]);
    for (start, end) in centered[0].into_iter().zip(centered[3]) {
        assert!(((start + end) * 0.5 - 100.5).abs() < 1e-5);
    }
    let horizontal = initial_points_with_modifiers((0, 0), (200, 9), true, false);
    assert_eq!(horizontal[0][1], horizontal[3][1]);
}
#[derive(serde::Deserialize)]
struct Spline {
    knots: [[f64; 2]; 4],
    segments: [[[f64; 2]; 3]; 3],
}
#[test]
fn natural_spline_matches_64_original_control_polygons() {
    let cases: Vec<Spline> =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-line-spline.json")).unwrap();
    assert_eq!(cases.len(), 64);
    for case in cases {
        let actual = spline_segments(case.knots);
        for (a, b) in actual
            .iter()
            .flatten()
            .flatten()
            .zip(case.segments.iter().flatten().flatten())
        {
            assert!((a - b).abs() < 1e-10, "{a} vs {b}");
        }
    }
}
#[test]
fn curve_modes_caps_dashes_alpha_selection_and_history() {
    let points = [[30., 70.], [70., 25.], [130., 120.], [180., 70.]];
    let mut modes = std::collections::HashSet::new();
    for mode in [CurveMode::Straight, CurveMode::Spline, CurveMode::Bezier] {
        let mut doc = Document::new(220, 150, [0; 4]).unwrap();
        doc.draw_curve(
            points,
            [220, 30, 70, 128],
            &LineOptions {
                mode,
                width: 4.,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(modes.insert(doc.layers[0].pixels.as_ref().clone()));
        assert!(doc.layers[0].pixels.iter().all(|p| p[3] <= 128));
        if mode == CurveMode::Spline {
            assert!(
                doc.layers[0].pixels[25 * 220 + 70][3] > 100,
                "Spline must pass through its knots"
            );
        }
    }
    let mut signatures = std::collections::HashSet::new();
    for cap in LineCapStyle::ALL {
        for dash in LineDash::ALL {
            let mut doc = Document::new(220, 150, [0; 4]).unwrap();
            doc.draw_curve(
                initial_points((35, 70), (180, 70)),
                [100, 120, 140, 128],
                &LineOptions {
                    width: 4.,
                    start_cap: cap,
                    end_cap: cap,
                    dash,
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(
                doc.layers[0].pixels.iter().any(|p| p[3] > 0),
                "{cap:?}/{dash:?}"
            );
            assert!(doc.layers[0].pixels.iter().all(|p| p[3] <= 128));
            assert!(
                signatures.insert(doc.layers[0].pixels.as_ref().clone()),
                "Duplicate cap/dash output {cap:?}/{dash:?}"
            );
        }
    }
    let mut doc = Document::new(220, 150, [0; 4]).unwrap();
    let mut mask = vec![0; 220 * 150];
    for y in 0..150 {
        for x in 80..150 {
            mask[y * 220 + x] = 128;
        }
    }
    doc.selection = Some(Arc::new(mask));
    let before = doc.clone();
    let mut editor = Editor::new(doc);
    editor
        .document
        .draw_curve(
            points,
            [50, 180, 70, 128],
            &LineOptions {
                width: 6.,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(editor.document.layers[0].pixels.iter().any(|p| p[3] == 64));
    assert!(
        editor.document.layers[0]
            .pixels
            .iter()
            .enumerate()
            .all(|(i, p)| p[3] <= 64 && (i % 220 >= 80 && i % 220 < 150 || p[3] == 0))
    );
    editor.record("Line / Curve", before.clone());
    let after = editor.document.clone();
    editor.undo();
    assert_eq!(editor.document, before);
    editor.redo();
    assert_eq!(editor.document, after);
    let mut invalid = LineOptions {
        width: f32::NAN,
        ..Default::default()
    };
    assert!(
        editor
            .document
            .draw_curve(points, [0; 4], &invalid)
            .is_err()
    );
    invalid.width = 1.;
    let mut bad = points;
    bad[1][0] = f32::INFINITY;
    assert!(editor.document.draw_curve(bad, [0; 4], &invalid).is_err());
    assert_eq!(editor.document, after);
}
