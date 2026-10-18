use openpdn::{
    document::Document,
    resolution::{ImageSize, MeasurementUnit, Resolution},
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[test]
fn new_image_uses_fresh_clipboard_pixels_or_the_normal_default() {
    for pixels in [[320, 240], [913, 517], [1, 729], [4096, 4096]] {
        let mut size = ImageSize::for_new(Some(pixels));
        assert_eq!(size.pixels(), pixels);
        assert_eq!(size.resolution(), 96.);
        assert_eq!(size.units(), MeasurementUnit::Inch);
        size.set_aspect(true);
        size.set_pixel_width(f64::from(pixels[0]) * 2.);
        assert_eq!(size.pixels(), pixels.map(|n| n * 2));
    }
    for pixels in [None, Some([0, 20]), Some([20, 0])] {
        assert_eq!(ImageSize::for_new(pixels).pixels(), [800, 600]);
    }
    // A recognizable image above the editor's pixel cap still supplies its
    // true dimensions. The New dialog then shows its size-validation error.
    let oversized = ImageSize::for_new(Some([6000, 4000]));
    assert_eq!(oversized.pixels(), [6000, 4000]);
    assert!(openpdn::document::pixel_count(6000, 4000).is_err());
    // A previously opened/edited New dialog must not replace the fallback with
    // stale clipboard dimensions or DPI on the next invocation.
    let mut previous = ImageSize::for_new(Some([913, 517]));
    previous.set_resolution(300.);
    previous.set_pixel_width(31.);
    let fresh = ImageSize::for_new(Some([245, 379]));
    assert_eq!(fresh.pixels(), [245, 379]);
    assert_eq!(fresh.resolution(), 96.);
    assert_eq!(ImageSize::for_new(None).pixels(), [800, 600]);
}

#[derive(Deserialize)]
struct Oracle {
    writes: Vec<Write>,
    #[serde(rename = "resizeCases")]
    resize_cases: Vec<Trace>,
}
#[derive(Deserialize)]
struct Write {
    unit: usize,
    value: f64,
    metadata: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct Trace {
    original: [u32; 2],
    operations: Vec<Operation>,
    states: Vec<State>,
}
#[derive(Deserialize)]
struct Operation {
    property: String,
    value: serde_json::Value,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct State {
    width: f64,
    height: f64,
    pixel_width: f64,
    pixel_height: f64,
    units: usize,
    resolution: f64,
    aspect: bool,
}
fn oracle() -> Oracle {
    serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-resolution.json")).unwrap()
}
fn compare(size: &ImageSize, state: &State) {
    for (got, want) in [
        (size.width(), state.width),
        (size.height(), state.height),
        (size.pixel_width(), state.pixel_width),
        (size.pixel_height(), state.pixel_height),
        (size.resolution(), state.resolution),
    ] {
        assert!(
            (got - want).abs() <= want.abs().max(1.) * 1e-13,
            "{got} != {want}"
        );
    }
    assert_eq!(size.units() as usize, state.units);
    assert_eq!(size.aspect(), state.aspect);
}
#[test]
fn original_resize_constrainer_traces() {
    let oracle = oracle();
    assert_eq!(oracle.resize_cases.len(), 128);
    for trace in oracle.resize_cases {
        let mut size = ImageSize::new(trace.original);
        compare(&size, &trace.states[0]);
        for (operation, state) in trace.operations.iter().zip(&trace.states[1..]) {
            let value = operation.value.as_f64();
            match operation.property.as_str() {
                "ConstrainToAspect" => size.set_aspect(operation.value.as_bool().unwrap()),
                "Resolution" => size.set_resolution(value.unwrap()),
                "NewPixelWidth" => size.set_pixel_width(value.unwrap()),
                "NewHeight" => size.set_height(value.unwrap()),
                "NewWidth" => size.set_width(value.unwrap()),
                "Units" => size.set_units(
                    MeasurementUnit::ALL[operation.value.as_u64().unwrap() as usize - 1],
                ),
                "SetByPercent" => size.set_by_percent(value.unwrap()),
                unknown => panic!("Unknown original operation {unknown}"),
            }
            compare(&size, state);
        }
    }
}
#[test]
fn original_resolution_writes_and_metadata_preservation() {
    let oracle = oracle();
    assert_eq!(oracle.writes.len(), 11);
    for write in oracle.writes {
        let mut document = Document::new(8, 6, [255; 4]).unwrap();
        document
            .set_resolution(Resolution {
                unit: MeasurementUnit::ALL[write.unit - 1],
                x: write.value,
                y: write.value,
            })
            .unwrap();
        // EXIF slot names are opaque allocation identities. The original Pixel
        // setter allocates X/Y twice; compare the emitted XML tags and bytes.
        let tags = |metadata: Vec<(String, String)>| -> BTreeMap<String, String> {
            metadata
                .into_iter()
                .map(|(_, xml)| {
                    let id = roxmltree::Document::parse(&xml)
                        .unwrap()
                        .root_element()
                        .attribute("id")
                        .unwrap()
                        .to_owned();
                    (id, xml)
                })
                .collect()
        };
        assert_eq!(
            tags(document.metadata),
            tags(write.metadata.into_iter().collect())
        );
    }
    let mut document =
        openpdn::pdn::read(include_bytes!("fixtures/pdn/small.pdn").as_slice()).unwrap();
    let opaque = (
        "$exif.tag0[1]".to_owned(),
        "<exif id=\"282\" len=\"8\" type=\"5\" value=\"AQAAAAEAAAA=\" />".to_owned(),
    );
    let profile = ("icc.profile".to_owned(), "opaque profile data".to_owned());
    document.metadata.extend([opaque.clone(), profile.clone()]);
    let desired = Resolution {
        unit: MeasurementUnit::Centimeter,
        x: 37.79527559055118,
        y: 42.25,
    };
    document.set_resolution(desired).unwrap();
    assert!(document.metadata.contains(&opaque));
    assert!(document.metadata.contains(&profile));
    assert_eq!(
        document.resolution(),
        Resolution {
            x: 37.795,
            ..desired
        }
    );
    let valid = document.metadata.clone();
    document.set_resolution(desired).unwrap();
    assert_eq!(document.metadata, valid);
    assert!(
        document
            .set_resolution(Resolution {
                x: f64::NAN,
                ..desired
            })
            .is_err()
    );
    assert_eq!(document.metadata, valid);
    let mut bytes = Vec::new();
    openpdn::pdn::write(&document, &mut bytes).unwrap();
    let loaded = openpdn::pdn::read(bytes.as_slice()).unwrap();
    assert_eq!(loaded.metadata, document.metadata);
    assert_eq!(loaded.resolution(), document.resolution());
}
