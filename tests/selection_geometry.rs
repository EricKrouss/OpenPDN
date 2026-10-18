use openpdn::{
    document::Document,
    resolution::{MeasurementUnit, convert_measurement},
    selection::SelectionMode,
    selection_geometry::{SelectionDrawMode, SelectionOptions, ellipse_bounds, rectangle_bounds},
};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    start: [f64; 2],
    end: [f64; 2],
    size: [f64; 2],
    shift: bool,
    normal: [f64; 4],
    ratio: [f64; 4],
    fixed_size: [f64; 4],
}
#[test]
fn original_rectangle_geometries() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-selection.json")).unwrap();
    assert_eq!(cases.len(), 256);
    for c in cases {
        for (mode, want) in [
            (SelectionDrawMode::Normal, c.normal),
            (SelectionDrawMode::FixedRatio, c.ratio),
            (SelectionDrawMode::FixedSize, c.fixed_size),
        ] {
            let got = rectangle_bounds(c.start, c.end, [800., 600.], mode, c.size, c.shift);
            for (a, b) in got.into_iter().zip(want) {
                assert!((a - b).abs() < 1e-10, "{mode:?}: {got:?} != {want:?}");
            }
        }
    }
}
#[derive(Deserialize)]
struct Conversion {
    length: f64,
    source: u16,
    basis: u16,
    dpu: f64,
    result: u16,
    value: f64,
}
#[derive(Deserialize)]
struct ResolutionOracle {
    metadata: std::collections::BTreeMap<String, String>,
    cases: Vec<Conversion>,
}
fn unit(value: u16) -> MeasurementUnit {
    MeasurementUnit::ALL[(value - 1) as usize]
}
#[test]
fn original_measurements_and_pdn_resolution() {
    let oracle: ResolutionOracle =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-resolution.json")).unwrap();
    assert_eq!(oracle.cases.len(), 117);
    for c in oracle.cases {
        assert_eq!(
            convert_measurement(
                c.length,
                unit(c.source),
                unit(c.basis),
                c.dpu,
                unit(c.result)
            ),
            c.value
        );
    }
    let mut doc = Document::new(800, 600, [255; 4]).unwrap();
    doc.metadata = oracle.metadata.into_iter().collect();
    let resolution = doc.resolution();
    assert_eq!((resolution.x, resolution.y), (144., 120.));
    assert_eq!(
        resolution.to_pixels([2., 3.], MeasurementUnit::Inch),
        [288., 360.]
    );
    let options = SelectionOptions {
        mode: SelectionDrawMode::FixedSize,
        width: 2.,
        height: 3.,
        units: MeasurementUnit::Inch,
    };
    let mask = doc.selection_shape_mask((0, 0), (799, 599), false, options, false);
    assert_eq!(mask.iter().filter(|&&p| p != 0).count(), 288 * 360);
    assert_eq!(mask[240 * 800 + 512], 255);
    assert_eq!(mask[239 * 800 + 512], 0);
    let mut stream = Vec::new();
    openpdn::pdn::write(&doc, &mut stream).unwrap();
    let reloaded = openpdn::pdn::read(std::io::Cursor::new(stream)).unwrap();
    assert_eq!(reloaded.resolution(), resolution);
    doc.metadata.retain(|(_, v)| !v.contains("id=\"283\""));
    assert_eq!(doc.resolution().y, 144.);
}
#[test]
fn selection_constraints_buttons_and_empty_union() {
    assert_eq!(
        ellipse_bounds([10., 10.], [16., 18.], true),
        [8., 9., 10., 10.]
    );
    let mut doc = Document::new(20, 20, [255; 4]).unwrap();
    let mask = doc.selection_shape_mask((1, 2), (8, 5), false, SelectionOptions::default(), true);
    assert_eq!(mask.iter().filter(|&&p| p != 0).count(), 16);
    doc.combine_selection(mask, SelectionMode::Add);
    assert_eq!(
        doc.selection
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&p| p != 0)
            .count(),
        16
    );
    assert_eq!(
        SelectionMode::Replace.from_pointer(false, true, true),
        SelectionMode::Add
    );
    assert_eq!(
        SelectionMode::Replace.from_pointer(true, true, true),
        SelectionMode::Xor
    );
    assert_eq!(
        SelectionMode::Replace.from_pointer(false, false, true),
        SelectionMode::Subtract
    );
    assert_eq!(
        SelectionMode::Replace.from_pointer(true, false, true),
        SelectionMode::Intersect
    );
    let options = SelectionOptions {
        mode: SelectionDrawMode::FixedSize,
        width: 30.,
        height: 40.,
        ..Default::default()
    };
    assert!(
        doc.selection_shape_mask((5, 5), (15, 15), false, options, false)
            .iter()
            .all(|&p| p == 255)
    );
}

#[test]
fn partial_selection_erase_and_invert_are_reversible() {
    use std::sync::Arc;
    let mut doc = Document::new(256, 1, [20, 40, 60, 200]).unwrap();
    let before = doc.clone();
    doc.erase_selection();
    assert_eq!(doc, before);
    doc.selection = Some(Arc::new((0..=255).collect()));
    doc.invert_selection();
    assert_eq!(&doc.selection.as_ref().unwrap()[..3], &[255, 254, 253]);
    doc.invert_selection();
    let before = doc.clone();
    let mut editor = openpdn::document::Editor::new(doc);
    editor.edit("Erase selection", Document::erase_selection);
    assert!(editor.document.selection.is_none());
    for (i, p) in editor.document.layers[0].pixels.iter().enumerate() {
        assert_eq!(
            *p,
            openpdn::tool_blend::ToolBlend::Overwrite.apply([20, 40, 60, 200], [0; 4], i as u8)
        );
    }
    editor.undo();
    assert_eq!(editor.document, before);
}
