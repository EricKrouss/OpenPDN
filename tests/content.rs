use openpdn::{content::*, document::*};
#[test]
fn text_document_resolution_and_fixed_metric_share_caret_and_raster_scale() {
    use openpdn::resolution::{MeasurementUnit, Resolution};
    let resolution = Resolution {
        unit: MeasurementUnit::Inch,
        x: 144.,
        y: 120.,
    };
    assert_eq!(font_size_dips(12., FontSizeMetric::Points, resolution), 24.);
    assert_eq!(
        font_size_dips(12., FontSizeMetric::Fixed96Dpi, resolution),
        16.
    );
    assert_eq!(
        font_size_dips(
            12.,
            FontSizeMetric::Points,
            Resolution {
                unit: MeasurementUnit::Centimeter,
                x: 144. / 2.54,
                y: 1.
            }
        ),
        24.
    );
    let mut renderer = TextRenderer::default();
    let mut high = Document::new(600, 400, TRANSPARENT).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-resolution.json")).unwrap();
    high.metadata = fixture["metadata"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
        .collect();
    let mut normal = Document::new(600, 400, TRANSPARENT).unwrap();
    let mut options = TextOptions {
        size: 32.,
        ..Default::default()
    };
    let text = "Resolution";
    let a = renderer.layout(text, &options).unwrap();
    let b = renderer
        .layout_at_resolution(text, &options, high.resolution())
        .unwrap();
    assert!((b.lines[0].height / a.lines[0].height - 1.5).abs() < 1e-5);
    renderer
        .render(&mut normal, text, (20, 20), [0, 0, 0, 255], &options)
        .unwrap();
    renderer
        .render(&mut high, text, (20, 20), [0, 0, 0, 255], &options)
        .unwrap();
    assert_ne!(normal.layers[0].pixels, high.layers[0].pixels);
    high.layers[0].pixels = std::sync::Arc::new(vec![TRANSPARENT; 600 * 400]);
    options.size_metric = FontSizeMetric::Fixed96Dpi;
    renderer
        .render(&mut high, text, (20, 20), [0, 0, 0, 255], &options)
        .unwrap();
    assert_eq!(normal.layers[0].pixels, high.layers[0].pixels);
}
#[test]
fn constrained_centered_and_subpixel_shapes() {
    for kind in ShapeKind::ALL {
        for end in [(170, 140), (-70, 140), (170, -40), (-70, -40)] {
            let (a, b) = shape_drawing_points((50, 50), end, kind, true, true);
            assert!(
                ((a.0 + b.0) / 2. - 50.).abs() < 1e-10 && ((a.1 + b.1) / 2. - 50.).abs() < 1e-10
            );
            assert!(
                ((b.0 - a.0).abs() / (b.1 - a.1).abs() - kind.aspect_ratio()).abs() < 1e-10,
                "{kind:?}"
            );
        }
    }
    assert_eq!(
        shape_drawing_points((10, 20), (100, 65), ShapeKind::Rectangle, true, false),
        ((10., 20.), (55., 65.))
    );
    assert!((ShapeKind::Triangle.aspect_ratio() - 2. / 3f64.sqrt()).abs() < 1e-12);
    assert!((ShapeKind::Hexagon.aspect_ratio() - 0.75f64.sqrt()).abs() < 1e-12);
    let mut fractional = Document::new(160, 120, TRANSPARENT).unwrap();
    let mut aligned = fractional.clone();
    let options = ShapeOptions {
        kind: ShapeKind::Ellipse,
        mode: ShapeMode::Fill,
        ..Default::default()
    };
    fractional
        .draw_shape_at(
            (20.25, 20.25),
            (100.25, 80.25),
            [20, 50, 120, 255],
            [0; 4],
            &options,
        )
        .unwrap();
    aligned
        .draw_shape((20, 20), (100, 80), [20, 50, 120, 255], [0; 4], &options)
        .unwrap();
    assert_ne!(
        fractional.layers[0].pixels, aligned.layers[0].pixels,
        "Subpixel bounds were rounded away"
    );
    let before = fractional.clone();
    assert!(
        fractional
            .draw_shape_at((f64::NAN, 0.), (30., 40.), [0; 4], [0; 4], &options)
            .is_err()
    );
    assert_eq!(fractional, before);
}

#[test]
fn text_works_without_installed_fonts() {
    let mut fonts = cosmic_text::FontSystem::new();
    let ids: Vec<_> = fonts.db().faces().map(|face| face.id).collect();
    for id in ids {
        fonts.db_mut().remove_face(id);
    }
    assert_eq!(fonts.db().faces().count(), 0);
    let mut renderer = TextRenderer::with_font_system(fonts);
    let mut doc = Document::new(300, 100, TRANSPARENT).unwrap();
    for (bold, italic) in [(false, false), (true, true)] {
        renderer
            .render(
                &mut doc,
                "Portable café",
                (10, 10),
                [20, 40, 70, 255],
                &TextOptions {
                    family: "missing font".into(),
                    bold,
                    italic,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(doc.layers[0].pixels.iter().any(|p| p[3] > 0));
    }
}

#[test]
fn text_and_shapes_scale_fractional_selection_alpha() {
    let mut full = Document::new(180, 90, TRANSPARENT).unwrap();
    let mut masked = full.clone();
    masked.selection = Some(std::sync::Arc::new(vec![128; 180 * 90]));
    let mut renderer = TextRenderer::default();
    let options = TextOptions::default();
    renderer
        .render(
            &mut full,
            "Coverage",
            (10, 10),
            [160, 70, 20, 128],
            &options,
        )
        .unwrap();
    renderer
        .render(
            &mut masked,
            "Coverage",
            (10, 10),
            [160, 70, 20, 128],
            &options,
        )
        .unwrap();
    for (a, b) in full.layers[0]
        .pixels
        .iter()
        .zip(masked.layers[0].pixels.iter())
    {
        assert_eq!(b[3], scale(u32::from(a[3]), 128) as u8);
    }
    let mut full = Document::new(180, 90, TRANSPARENT).unwrap();
    let mut masked = full.clone();
    masked.selection = Some(std::sync::Arc::new(vec![128; 180 * 90]));
    let options = ShapeOptions {
        kind: ShapeKind::Ellipse,
        mode: ShapeMode::Both,
        ..Default::default()
    };
    for doc in [&mut full, &mut masked] {
        doc.draw_shape(
            (10, 10),
            (160, 80),
            [200, 40, 30, 128],
            [30, 40, 200, 255],
            &options,
        )
        .unwrap();
    }
    for (a, b) in full.layers[0]
        .pixels
        .iter()
        .zip(masked.layers[0].pixels.iter())
    {
        assert_eq!(b[3], scale(u32::from(a[3]), 128) as u8);
    }
}

#[test]
fn all_shape_presets_have_distinct_renderable_geometry_and_clip_to_selection() {
    let mut signatures = std::collections::HashSet::new();
    for kind in ShapeKind::ALL {
        let mut doc = Document::new(160, 120, TRANSPARENT).unwrap();
        let options = ShapeOptions {
            kind,
            mode: ShapeMode::Both,
            width: 3.5,
            ..Default::default()
        };
        doc.select_rect((10, 10), (140, 100), false);
        doc.draw_shape(
            (10, 10),
            (150, 110),
            [220, 20, 30, 255],
            [20, 100, 230, 128],
            &options,
        )
        .unwrap();
        let pixels = &doc.layers[0].pixels;
        assert!(
            pixels.iter().any(|p| p[3] > 0),
            "{} produced no pixels",
            kind.name()
        );
        assert!(
            pixels
                .iter()
                .enumerate()
                .all(|(i, p)| doc.selected(i) || *p == TRANSPARENT)
        );
        assert!(
            signatures.insert(pixels.as_ref().clone()),
            "{} duplicates another preset",
            kind.name()
        );
    }
}
#[test]
fn shape_fill_outline_hole_rotation_opacity_and_undo() {
    let mut doc = Document::new(120, 120, TRANSPARENT).unwrap();
    let mut options = ShapeOptions {
        kind: ShapeKind::Gear,
        mode: ShapeMode::Fill,
        ..Default::default()
    };
    doc.draw_shape((10, 10), (110, 110), [200, 10, 20, 128], [0; 4], &options)
        .unwrap();
    assert_eq!(
        doc.layers[0].pixels[60 * 120 + 60],
        TRANSPARENT,
        "Gear hole must remain empty"
    );
    assert!(doc.layers[0].pixels.iter().all(|p| p[3] <= 128));
    let mut editor = Editor::new(doc);
    let before = editor.document.clone();
    options.kind = ShapeKind::Rectangle;
    options.mode = ShapeMode::Outline;
    options.dashed = true;
    options.angle = 25.;
    editor
        .document
        .draw_shape((20, 30), (90, 80), [0, 0, 0, 255], [0; 4], &options)
        .unwrap();
    editor.record("Shapes", before.clone());
    editor.undo();
    assert_eq!(editor.document.layers[0].pixels, before.layers[0].pixels);
    options.width = f32::NAN;
    assert!(
        editor
            .document
            .draw_shape((0, 0), (10, 10), [0; 4], [0; 4], &options)
            .is_err()
    );
}
#[test]
fn unicode_text_style_alignment_alpha_selection_and_undo() {
    let mut renderer = TextRenderer::default();
    assert!(!renderer.families.is_empty());
    let mut doc = Document::new(500, 220, TRANSPARENT).unwrap();
    let mut options = TextOptions {
        size: 24.,
        bold: true,
        italic: true,
        underline: true,
        strikeout: true,
        ..Default::default()
    };
    let text = "Rust café\nمرحبا שלום 日本語 🦀";
    renderer
        .render(&mut doc, text, (20, 15), [180, 30, 90, 128], &options)
        .unwrap();
    assert!(doc.layers[0].pixels.iter().any(|p| p[3] > 0));
    assert!(
        doc.layers[0].pixels.iter().all(|p| p[3] <= 128),
        "Text must honor primary alpha, including emoji"
    );
    let mut centered = Document::new(500, 220, TRANSPARENT).unwrap();
    options.align = TextAlign::Center;
    options.antialias = false;
    centered.select_rect((200, 0), (499, 219), false);
    renderer
        .render(
            &mut centered,
            "Hello",
            (250, 15),
            [10, 20, 30, 255],
            &options,
        )
        .unwrap();
    assert!(
        centered.layers[0]
            .pixels
            .iter()
            .enumerate()
            .all(|(i, p)| centered.selected(i) || *p == TRANSPARENT)
    );
    assert!(
        centered.layers[0]
            .pixels
            .iter()
            .all(|p| p[3] == 0 || p[3] == 255)
    );
    let mut editor = Editor::new(centered);
    let before = editor.document.clone();
    renderer
        .render(
            &mut editor.document,
            "More",
            (250, 80),
            [0, 0, 0, 255],
            &options,
        )
        .unwrap();
    editor.record("Text", before.clone());
    editor.undo();
    assert_eq!(editor.document.layers[0].pixels, before.layers[0].pixels);
    let blank = Document::new(20, 20, TRANSPARENT).unwrap();
    let mut invisible = blank.clone();
    renderer
        .render(&mut invisible, "A", (0, 0), [0; 4], &TextOptions::default())
        .unwrap();
    assert_eq!(invisible.layers[0].pixels, blank.layers[0].pixels);
    options.size = f32::INFINITY;
    assert!(
        renderer
            .render(&mut doc, "A", (0, 0), [0; 4], &options)
            .is_err()
    );
}
