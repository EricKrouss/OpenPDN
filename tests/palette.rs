use openpdn::{document::Pixel, palette};
use serde::Deserialize;
#[derive(Deserialize)]
struct Case {
    input: String,
    result: Vec<Pixel>,
}
#[derive(Deserialize)]
struct HsvCase {
    rgb: [u8; 3],
    hsv: [u16; 3],
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Oracle {
    defaults: Vec<Pixel>,
    cases: Vec<Case>,
    rgb_cases: Vec<HsvCase>,
    hsv_cases: Vec<HsvCase>,
    wheel_cases: Vec<WheelCase>,
    wheel_pixels: Vec<WheelPixel>,
    file_cases: Vec<FileCase>,
    padded_cases: Vec<PaddedCase>,
}
#[derive(Deserialize)]
struct PaddedCase {
    input: Vec<Pixel>,
    result: Vec<Pixel>,
}
#[derive(Deserialize)]
struct FileCase {
    bytes: Vec<u8>,
    result: Vec<Pixel>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WheelCase {
    point: [f64; 2],
    basis: [u16; 3],
    snap: bool,
    lock_hue: bool,
    lock_radius: bool,
    result: [u16; 3],
}
#[derive(Deserialize)]
struct WheelPixel {
    point: [f64; 2],
    result: Pixel,
}
#[test]
fn original_palette_and_integer_color_controls() {
    let oracle: Oracle =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-palettes.json")).unwrap();
    assert_eq!(oracle.defaults, palette::DEFAULT);
    assert_eq!(oracle.padded_cases.len(), 6);
    for c in oracle.padded_cases {
        assert_eq!(palette::library_colors(&c.input), c.result);
    }
    assert_eq!(oracle.cases.len(), 25);
    for c in oracle.cases {
        assert_eq!(palette::parse(&c.input), c.result, "{:?}", c.input);
    }
    assert_eq!(oracle.rgb_cases.len(), 1024);
    assert_eq!(oracle.hsv_cases.len(), 1024);
    for c in oracle.rgb_cases {
        assert_eq!(
            palette::rgb_to_hsv([c.rgb[0], c.rgb[1], c.rgb[2], 255]),
            c.hsv
        );
    }
    for c in oracle.hsv_cases {
        assert_eq!(palette::hsv_to_rgb(c.hsv), c.rgb, "{:?}", c.hsv);
    }
    assert_eq!(oracle.wheel_cases.len(), 512);
    assert_eq!(oracle.wheel_pixels.len(), 512);
    for c in oracle.wheel_cases {
        assert_eq!(
            openpdn::color_wheel::grab_color(
                c.point.map(|v| v - 68.),
                68.,
                c.basis,
                openpdn::color_wheel::WheelModifiers {
                    snap: c.snap,
                    lock_hue: c.lock_hue,
                    lock_radius: c.lock_radius
                }
            ),
            c.result
        );
    }
    for c in oracle.wheel_pixels {
        assert_eq!(openpdn::color_wheel::wheel_pixel(c.point, 68.), c.result);
    }
    assert_eq!(oracle.file_cases.len(), 8);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("original-encoding.txt");
    for c in oracle.file_cases {
        std::fs::write(&file, c.bytes).unwrap();
        assert_eq!(palette::load(&file).unwrap(), c.result);
    }
}
#[test]
fn palette_file_round_trip_preserves_alpha_and_comments() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("colors.txt");
    let colors = vec![[1, 2, 3, 0], [4, 5, 6, 127], [7, 8, 9, 255]];
    palette::save(&path, &colors).unwrap();
    assert_eq!(palette::load(&path).unwrap(), colors);
    std::fs::write(
        &path,
        "\u{feff}; a comment\r\nff123456 ; custom\r\nmalformed\n123456\n",
    )
    .unwrap();
    assert_eq!(
        palette::load(&path).unwrap(),
        vec![[18, 52, 86, 255], [18, 52, 86, 0]]
    );
    assert_eq!(palette::parse_rgb_hex("#010203"), Some([1, 2, 3]));
    assert_eq!(palette::parse_rgb_hex("ABC"), Some([0, 10, 188]));
    assert_eq!(palette::parse_rgb_hex("FF010203"), None);
}

#[test]
fn named_library_reads_both_locations_and_replaces_without_case_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let library = palette::Library {
        user_dir: dir.path().join("user"),
        app_dir: Some(dir.path().join("app")),
    };
    assert!(library.entries().unwrap().is_empty());
    let colors = [[1, 2, 3, 127], [4, 5, 6, 0]];
    let saved = library.save_named("My Palette", &colors).unwrap();
    assert_eq!(saved.file_name().unwrap(), "My Palette.txt");
    let app = library.app_dir.as_ref().unwrap();
    std::fs::create_dir(app).unwrap();
    palette::save(&app.join("My Palette.TXT"), &[[20, 30, 40, 50]]).unwrap();
    std::fs::write(library.user_dir.join("unrelated.png"), [1, 2, 3]).unwrap();
    let entries = library.entries().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].location, palette::Location::User);
    assert_eq!(entries[0].colors, palette::library_colors(&colors));
    assert_eq!(entries[1].location, palette::Location::App);
    assert_eq!(entries[1].colors[0], [20, 30, 40, 50]);
    assert_eq!(
        library.save_named("my palette.png", &[[9; 4]]).unwrap(),
        saved
    );
    assert_eq!(library.entries().unwrap().len(), 2);
    assert_eq!(palette::load(&saved).unwrap()[0], [9; 4]);
    for bad in ["", "../outside.txt", "a/b.txt", "a\\b.txt", "a:b", "a\0b"] {
        assert!(library.save_named(bad, &colors).is_err(), "{bad:?}");
    }
    assert!(!dir.path().join("outside.txt").exists());
    let duplicate = library.user_dir.join("MY PALETTE.TXT");
    // Case-insensitive filesystems cannot contain both spellings. Exercise a
    // duplicate only when this host permits two separate directory entries.
    if !duplicate.exists() {
        palette::save(&duplicate, &colors).unwrap();
        assert!(library.entries().is_err());
        std::fs::remove_file(duplicate).unwrap();
    }
    // A failed read is reported; it cannot silently replace a valid library.
    std::fs::write(
        library.user_dir.join("oversize.txt"),
        vec![0; 1024 * 1024 + 1],
    )
    .unwrap();
    assert!(library.entries().is_err());
    assert_eq!(palette::load(&saved).unwrap()[0], [9; 4]);
}
