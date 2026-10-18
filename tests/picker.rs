use openpdn::{
    document::{Document, Pixel},
    picker::{SampleSize, average},
};
use serde::Deserialize;
use std::sync::Arc;
#[derive(Deserialize)]
struct Case {
    input: Vec<Pixel>,
    result: Pixel,
}
#[test]
fn original_average_color_float_order_and_rounding() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-picker.json")).unwrap();
    assert_eq!(cases.len(), 256);
    for (i, case) in cases.into_iter().enumerate() {
        assert_eq!(average(case.input.into_iter()), case.result, "case {i}");
    }
}
#[test]
fn picker_sizes_clip_edges_ignore_selection_and_sample_active_or_merged() {
    let mut doc = Document::new(4, 4, [0, 0, 255, 255]).unwrap();
    Arc::make_mut(&mut doc.layers[0].pixels)[0] = [255, 0, 0, 0];
    assert_eq!(
        doc.sample_color((0, 0), SampleSize::Point, false),
        Some([255, 0, 0, 0])
    );
    assert_eq!(
        doc.sample_color((0, 0), SampleSize::Three, false),
        Some([0, 0, 255, 191])
    );
    doc.selection = Some(Arc::new(vec![0; 16]));
    doc.layers.push(
        Document::new(4, 4, [255, 0, 0, 128])
            .unwrap()
            .layers
            .remove(0),
    );
    doc.active = 1;
    let before = doc.clone();
    for size in SampleSize::ALL {
        assert_eq!(
            doc.sample_color((1, 1), size, false),
            Some([255, 0, 0, 128])
        );
        assert_eq!(doc.sample_color((-1, 1), size, true), None);
        assert_eq!(doc.sample_color((4, 1), size, false), None);
        let expected = doc.composite();
        if size == SampleSize::Point {
            assert_eq!(doc.sample_color((2, 2), size, true), Some(expected[10]));
        } else if size.pixels() >= 11 {
            assert_eq!(
                doc.sample_color((2, 2), size, true),
                Some(average(expected.into_iter()))
            );
        }
    }
    assert_eq!(doc, before);
    assert_eq!(SampleSize::ALL.map(|s| s.pixels()), [1, 3, 5, 11, 31, 51]);
}
