use openpdn::{document::*, tonal::*};
use serde::Deserialize;
use std::sync::Arc;
#[derive(Deserialize)]
struct Fixture {
    levels: Vec<LevelCase>,
    curves: Vec<CurveCase>,
}
#[derive(Deserialize)]
struct LevelCase {
    input_black: [f32; 3],
    input_white: [f32; 3],
    gamma: [f32; 3],
    output_black: [f32; 3],
    output_white: [f32; 3],
    table: Vec<[f32; 3]>,
}
#[derive(Deserialize)]
struct CurveCase {
    points: Vec<(u8, u8)>,
    values: Vec<f32>,
}
#[test]
fn levels_and_splines_match_original_managed_outputs() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-tonal.json")).unwrap();
    for case in fixture.levels {
        let levels = Levels {
            input_black: case.input_black,
            input_white: case.input_white,
            gamma: case.gamma,
            output_black: case.output_black,
            output_white: case.output_white,
        };
        let actual = levels.table().unwrap();
        for (i, (a, e)) in actual.iter().zip(case.table).enumerate() {
            for c in 0..3 {
                assert!(
                    (a[c] - e[c]).abs() <= 0.000001,
                    "Levels sample {i} channel {c}: {} vs {}",
                    a[c],
                    e[c]
                );
            }
        }
    }
    for case in fixture.curves {
        let actual = spline(&case.points, 1024).unwrap();
        for (i, (a, e)) in actual.iter().zip(case.values).enumerate() {
            assert!((a - e).abs() <= 0.000001, "Spline sample {i}: {a} vs {e}");
        }
    }
}
#[test]
fn auto_levels_uses_selected_histogram_and_handles_constant_empty_and_invalid_input() {
    let mut doc = Document::new(256, 1, TRANSPARENT).unwrap();
    doc.layers[0].pixels = Arc::new((0..=255).map(|v| [v, v, v, 255]).collect());
    doc.select_rect((64, 0), (192, 0), false);
    let auto = Levels::auto(&doc);
    assert_eq!(auto.input_black, [64. / 255.; 3]);
    assert_eq!(auto.input_white, [192. / 255.; 3]);
    assert!((auto.gamma[0] - 1.).abs() < 0.000001);
    assert_eq!(auto.table().unwrap()[64][0], 0.);
    assert_eq!(auto.table().unwrap()[192][0], 1.);
    let empty = Document::new(2, 2, [100; 4]).unwrap();
    let auto = Levels::auto(&empty);
    assert_eq!(auto.gamma, [1.; 3]);
    assert!(
        auto.table()
            .unwrap()
            .iter()
            .flatten()
            .all(|v| v.is_finite())
    );
    let mut none = empty.clone();
    none.selection = Some(Arc::new(vec![0; 4]));
    assert_eq!(
        Levels::auto(&none).table().unwrap(),
        Levels::default().table().unwrap()
    );
    assert!(spline(&[(0, 0), (0, 1)], 256).is_err());
    assert!(spline(&[(0, 0)], 256).is_err());
    let invalid = Levels {
        gamma: [f32::NAN; 3],
        ..Default::default()
    };
    assert!(invalid.table().is_err());
}
