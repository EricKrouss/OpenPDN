use openpdn::{document::*, selection::*};
use serde::Deserialize;
use std::sync::Arc;
#[derive(Deserialize)]
struct Fixture {
    a: Pixel,
    b: Pixel,
    premultiplied: u8,
    straight: u8,
}
#[test]
fn original_flood_distances_and_transparent_hidden_rgb() {
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-flood.json")).unwrap();
    for f in fixtures {
        assert_eq!(
            color_distance(f.a, f.b, AlphaMode::Premultiplied),
            f.premultiplied,
            "premultiplied {:?} {:?}",
            f.a,
            f.b
        );
        assert_eq!(
            color_distance(f.a, f.b, AlphaMode::Straight),
            f.straight,
            "straight {:?} {:?}",
            f.a,
            f.b
        );
    }
    assert_eq!(
        color_distance([255, 0, 0, 0], [0, 255, 0, 0], AlphaMode::Premultiplied),
        0
    );
    assert_ne!(
        color_distance([255, 0, 0, 0], [0, 255, 0, 0], AlphaMode::Straight),
        0
    );
}

#[test]
fn flood_tolerance_stays_seed_relative_and_respects_alpha_modes() {
    // Matching the previous pixel would leak all the way across this ramp.
    let ramp: Vec<Pixel> = (0..6).map(|x| [x * 20, 0, 0, 255]).collect();
    for flood in [FloodMode::Contiguous, FloodMode::Global] {
        assert_eq!(
            flood_mask(&ramp, 6, (0, 0), 10, flood, AlphaMode::Straight),
            [255, 255, 0, 0, 0, 0]
        );
        assert_eq!(
            flood_mask(&ramp, 6, (0, 0), 9, flood, AlphaMode::Straight),
            [255, 0, 0, 0, 0, 0]
        );
    }
    let transparent = [
        [255, 0, 0, 0],
        [0, 255, 0, 0],
        [255, 0, 0, 128],
        [255, 0, 0, 255],
        [0, 0, 255, 0],
    ];
    assert_eq!(
        flood_mask(
            &transparent,
            5,
            (0, 0),
            0,
            FloodMode::Contiguous,
            AlphaMode::Premultiplied
        ),
        [255, 255, 0, 0, 0]
    );
    assert_eq!(
        flood_mask(
            &transparent,
            5,
            (0, 0),
            0,
            FloodMode::Global,
            AlphaMode::Premultiplied
        ),
        [255, 255, 0, 0, 255]
    );
    assert_eq!(
        flood_mask(
            &transparent,
            5,
            (0, 0),
            0,
            FloodMode::Global,
            AlphaMode::Straight
        ),
        [255, 0, 0, 0, 0]
    );
}

// Deliberately pixel-at-a-time: an independent oracle for the span traversal.
fn reachable(
    pixels: &[Pixel],
    width: usize,
    seed: usize,
    tolerance: u8,
    alpha: AlphaMode,
) -> Vec<u8> {
    let mut out = vec![0; pixels.len()];
    let mut pending = std::collections::VecDeque::from([seed]);
    out[seed] = 255;
    while let Some(i) = pending.pop_front() {
        for j in 0..pixels.len() {
            let adjacent = i / width == j / width && i.abs_diff(j) == 1 || i.abs_diff(j) == width;
            if adjacent
                && out[j] == 0
                && color_distance(pixels[seed], pixels[j], alpha) <= tolerance
            {
                out[j] = 255;
                pending.push_back(j);
            }
        }
    }
    out
}

#[test]
fn scanline_flood_preserves_thin_barriers_holes_and_four_way_connectivity() {
    // All 512 binary 3x3 images, from every seed, cover diagonal contacts,
    // one-pixel barriers, holes, split runs and reconnection around obstacles.
    for bits in 0..512 {
        let pixels: Vec<Pixel> = (0..9)
            .map(|i| {
                if bits & (1 << i) == 0 {
                    [255; 4]
                } else {
                    [0, 0, 0, 255]
                }
            })
            .collect();
        for seed in 0..9 {
            assert_eq!(
                flood_mask(
                    &pixels,
                    3,
                    (seed as i32 % 3, seed as i32 / 3),
                    0,
                    FloodMode::Contiguous,
                    AlphaMode::Premultiplied
                ),
                reachable(&pixels, 3, seed, 0, AlphaMode::Premultiplied),
                "layout {bits:09b}, seed {seed}"
            );
        }
    }
    // Mixed alpha and gradients also exercise the original distance threshold,
    // including transparent hidden RGB and partially matching adjacent runs.
    let palette = [
        [0, 0, 0, 255],
        [20, 0, 0, 255],
        [40, 0, 0, 255],
        [128, 64, 32, 128],
        [128, 80, 32, 128],
        [255, 0, 0, 0],
        [0, 255, 0, 0],
        [255; 4],
    ];
    let mut rng = 5112u32;
    for _ in 0..64 {
        let pixels: Vec<Pixel> = (0..63)
            .map(|_| {
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                palette[(rng >> 24) as usize % palette.len()]
            })
            .collect();
        for alpha in [AlphaMode::Premultiplied, AlphaMode::Straight] {
            for tolerance in [0, 9, 10, 32, 127, 255] {
                let seed = (rng as usize) % pixels.len();
                assert_eq!(
                    flood_mask(
                        &pixels,
                        9,
                        (seed as i32 % 9, seed as i32 / 9),
                        tolerance,
                        FloodMode::Contiguous,
                        alpha
                    ),
                    reachable(&pixels, 9, seed, tolerance, alpha)
                );
                let global: Vec<_> = pixels
                    .iter()
                    .map(|&p| {
                        if color_distance(pixels[seed], p, alpha) <= tolerance {
                            255
                        } else {
                            0
                        }
                    })
                    .collect();
                assert_eq!(
                    flood_mask(
                        &pixels,
                        9,
                        (seed as i32 % 9, seed as i32 / 9),
                        tolerance,
                        FloodMode::Global,
                        alpha
                    ),
                    global
                );
            }
        }
    }
}

#[test]
fn flood_validates_image_bounds_and_handles_long_runs() {
    for flood in [FloodMode::Contiguous, FloodMode::Global] {
        for (pixels, width, point) in [
            (vec![], 0, (0, 0)),
            (vec![[255; 4]; 4], 0, (0, 0)),
            (vec![[255; 4]; 4], 3, (0, 0)),
            (vec![[255; 4]; 4], 2, (-1, 0)),
            (vec![[255; 4]; 4], 2, (0, -1)),
            (vec![[255; 4]; 4], 2, (2, 0)),
            (vec![[255; 4]; 4], 2, (0, 2)),
            (vec![[255; 4]; 4], 2, (i32::MAX, i32::MAX)),
        ] {
            assert_eq!(
                flood_mask(&pixels, width, point, 255, flood, AlphaMode::Premultiplied),
                vec![0; pixels.len()]
            );
        }
    }
    for (width, height) in [(1, 1), (1024, 1), (1, 1024), (1024, 513)] {
        let pixels = vec![[64, 128, 192, 255]; width * height];
        assert_eq!(
            flood_mask(
                &pixels,
                width as u32,
                ((width - 1) as i32, (height / 2) as i32),
                0,
                FloodMode::Contiguous,
                AlphaMode::Premultiplied
            ),
            vec![255; pixels.len()]
        );
    }
}
#[test]
fn flood_connectivity_global_sampling_and_selection_algebra() {
    let white = [255; 4];
    let black = [0, 0, 0, 255];
    let mut doc = Document::new(5, 3, white).unwrap();
    for y in 0..3 {
        Arc::make_mut(&mut doc.layers[0].pixels)[y * 5 + 2] = black;
    }
    doc.magic_wand(
        (0, 0),
        0,
        FloodMode::Contiguous,
        AlphaMode::Premultiplied,
        false,
        SelectionMode::Replace,
    );
    assert_eq!(
        doc.selection
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&v| v != 0)
            .count(),
        6
    );
    doc.magic_wand(
        (4, 0),
        0,
        FloodMode::Contiguous,
        AlphaMode::Premultiplied,
        false,
        SelectionMode::Add,
    );
    assert_eq!(
        doc.selection
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&v| v != 0)
            .count(),
        12
    );
    doc.magic_wand(
        (4, 0),
        0,
        FloodMode::Contiguous,
        AlphaMode::Premultiplied,
        false,
        SelectionMode::Subtract,
    );
    assert_eq!(
        doc.selection
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&v| v != 0)
            .count(),
        6
    );
    doc.magic_wand(
        (4, 0),
        0,
        FloodMode::Global,
        AlphaMode::Premultiplied,
        false,
        SelectionMode::Replace,
    );
    assert_eq!(
        doc.selection
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&v| v != 0)
            .count(),
        12
    );
    doc.add_layer();
    doc.magic_wand(
        (0, 0),
        0,
        FloodMode::Contiguous,
        AlphaMode::Premultiplied,
        true,
        SelectionMode::Replace,
    );
    assert_eq!(
        doc.selection
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&v| v != 0)
            .count(),
        6
    );
    doc.fill_with_options(
        (0, 0),
        [200, 20, 40, 255],
        0,
        FloodMode::Global,
        AlphaMode::Premultiplied,
        true,
    );
    assert_eq!(doc.layers[1].pixels[0], [200, 20, 40, 255]);
    assert_eq!(doc.layers[1].pixels[4], TRANSPARENT);
    doc.combine_selection(vec![255; 15], SelectionMode::Xor);
    assert_eq!(
        doc.selection
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&v| v != 0)
            .count(),
        9
    );
}
#[test]
fn lasso_and_overlapping_pixel_translation_are_undoable() {
    let mut editor = Editor::new(Document::new(6, 5, TRANSPARENT).unwrap());
    Arc::make_mut(&mut editor.document.layers[0].pixels)[7] = [60, 90, 120, 128];
    editor.edit("Lasso", |d| {
        d.select_polygon(&[(1, 1), (4, 1), (4, 4), (1, 4)], SelectionMode::Replace)
    });
    let before = editor.document.clone();
    assert_eq!(
        before
            .selection
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&v| v != 0)
            .count(),
        9
    );
    editor.edit("Move pixels", |d| d.translate_selection(1, 0, true));
    assert_eq!(editor.document.layers[0].pixels[7], TRANSPARENT);
    assert_eq!(editor.document.layers[0].pixels[8], [60, 90, 120, 128]);
    editor.undo();
    assert_eq!(editor.document, before);
    editor.redo();
    let pixels = editor.document.layers[0].pixels.clone();
    editor.edit("Move selection", |d| d.translate_selection(0, -2, false));
    assert_eq!(editor.document.layers[0].pixels, pixels);
    assert_eq!(
        editor
            .document
            .selection
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&v| v != 0)
            .count(),
        6
    );
}
