use openpdn::{
    document::{BlendMode, Document, Editor, blend},
    editing::{Anchor, ClipboardImage},
    gpu::{Gpu, Resampler},
};
use std::sync::Arc;
fn fixture() -> Document {
    let mut d = Document::new(5, 3, [0; 4]).unwrap();
    d.layers[0].pixels = Arc::new(
        (0..15)
            .map(|i| {
                [
                    (i * 15) as u8,
                    (i * 7) as u8,
                    100,
                    if i % 3 == 0 { 0 } else { 255 },
                ]
            })
            .collect(),
    );
    d.metadata.push(("test".into(), "preserved".into()));
    d.add_layer();
    d.layers[1].pixels = Arc::new(vec![[240, 20, 50, 128]; 15]);
    d.layers[1].name = "upper".into();
    d.layers[1].opacity = 173;
    d.layers[1].blend_mode = BlendMode::Multiply;
    d
}
#[test]
fn canvas_anchors_crop_and_history() {
    for anchor in Anchor::ALL {
        for (width, height) in [(8, 6), (2, 2), (5, 3)] {
            let original = fixture();
            let mut editor = Editor::new(original.clone());
            let [dx, dy] = anchor.offset([5, 3], [width, height]);
            editor.edit("Canvas Size", |d| {
                d.canvas_size(width, height, anchor, [3, 4, 5, 6]).unwrap()
            });
            let d = &editor.document;
            assert_eq!(d.metadata, original.metadata);
            for (layer, old) in d.layers.iter().zip(&original.layers) {
                assert_eq!(layer.name, old.name);
                assert_eq!(layer.opacity, old.opacity);
                assert_eq!(layer.blend_mode, old.blend_mode);
                for y in 0..height as i32 {
                    for x in 0..width as i32 {
                        let (ox, oy) = (x - dx, y - dy);
                        let expected = if (0..5).contains(&ox) && (0..3).contains(&oy) {
                            old.pixels[(oy * 5 + ox) as usize]
                        } else {
                            [3, 4, 5, 6]
                        };
                        assert_eq!(layer.pixels[(y * width as i32 + x) as usize], expected);
                    }
                }
            }
            editor.undo();
            assert_eq!(editor.document, original);
        }
    }
    let mut d = fixture();
    d.selection = Some(Arc::new(vec![
        0, 0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 128, 255, 0, 0,
    ]));
    let before = d.clone();
    let mut editor = Editor::new(d);
    editor.edit("Crop", Document::crop_to_selection);
    assert_eq!((editor.document.width, editor.document.height), (3, 2));
    assert!(editor.document.selection.is_none());
    assert_eq!(
        editor.document.layers[1]
            .pixels
            .iter()
            .map(|v| v[3])
            .collect::<Vec<_>>(),
        [128, 0, 128, 64, 128, 0]
    );
    editor.undo();
    assert_eq!(editor.document, before);
    editor.redo();
    assert_eq!(editor.history_position(), 1);
    let before = editor.document.clone();
    assert!(
        editor
            .document
            .canvas_size(0, 20, Anchor::Center, [0; 4])
            .is_err()
    );
    assert_eq!(editor.document, before);
}
#[test]
fn merge_keeps_lower_properties_ignores_visibility_and_selection() {
    let mut d = fixture();
    d.layers[0].opacity = 63;
    d.layers[0].blend_mode = BlendMode::Screen;
    d.layers[0].visible = false;
    d.layers[1].visible = false;
    d.selection = Some(Arc::new(vec![0; 15]));
    let original = d.clone();
    let mut editor = Editor::new(d);
    editor.edit("Merge Down", Document::merge_down);
    assert_eq!(editor.document.layers.len(), 1);
    assert_eq!(editor.document.active, 0);
    let l = &editor.document.layers[0];
    assert_eq!(l.opacity, 63);
    assert_eq!(l.blend_mode, BlendMode::Screen);
    assert!(!l.visible);
    for i in 0..15 {
        assert_eq!(
            l.pixels[i],
            blend(
                original.layers[0].pixels[i],
                original.layers[1].pixels[i],
                173,
                BlendMode::Multiply
            )
        );
    }
    editor.undo();
    assert_eq!(editor.document, original);
    editor.redo();
    let count = editor.history_position();
    editor.edit("Merge", Document::merge_down);
    assert_eq!(editor.history_position(), count);
}
#[test]
fn clipboard_selection_merged_alpha_paste_and_undo() {
    let mut d = fixture();
    d.select_rect((1, 1), (3, 2), false);
    Arc::make_mut(d.selection.as_mut().unwrap())[7] = 0;
    let image = d.copy_selection(false).unwrap();
    assert_eq!((image.width, image.height, image.origin), (3, 2, [1, 1]));
    assert_eq!(image.pixels[1][3], 0);
    let merged = d.copy_selection(true).unwrap();
    assert_ne!(merged.pixels, image.pixels);
    let mut editor = Editor::new(d.clone());
    editor.edit("Paste", |d| d.paste_image(&image, [-1, 0], true).unwrap());
    assert_eq!(editor.document.layers.len(), 3);
    assert_eq!(editor.document.layers[2].pixels[1], image.pixels[2]);
    assert!(!editor.document.selected(4));
    assert!(editor.document.selected(0));
    editor.undo();
    assert_eq!(editor.document, d);
    editor.redo();
    assert_eq!(editor.document.layers.len(), 3);
    let pasted = image.clone().into_document().unwrap();
    assert_eq!(*pasted.layers[0].pixels, image.pixels);
    let mut invalid = ClipboardImage {
        width: 2,
        height: 2,
        pixels: vec![],
        origin: [0, 0],
    };
    assert!(
        editor
            .document
            .paste_image(&invalid, [0, 0], false)
            .is_err()
    );
    invalid.width = 0;
    assert!(invalid.into_document().is_err());
}
#[test]
#[ignore = "Requires an explicitly verified hardware adapter"]
fn hardware_resize_alpha_gamma_sampling_geometry_and_history() {
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert_ne!(gpu.info.device_type, eframe::wgpu::DeviceType::Cpu);
    eprintln!("Resize hardware: {} {:?}", gpu.info.name, gpu.info.backend);
    let d = fixture();
    for method in Resampler::ALL {
        for linear in [false, true] {
            for (w, h) in [(2, 7), (9, 1), (1, 1), (13, 11)] {
                let out = gpu.resize_document(&d, w, h, method, linear).unwrap();
                assert_eq!((out.width, out.height), (w, h));
                assert_eq!(out.metadata, d.metadata);
                assert_eq!(out.layers[1].name, "upper");
                for p in out.layers[1].pixels.iter() {
                    for (a, b) in p.iter().zip([240, 20, 50, 128]) {
                        assert!(a.abs_diff(b) <= 1, "{method:?} {p:?}");
                    }
                }
                if method == Resampler::Nearest {
                    for layer in 0..2 {
                        for y in 0..h {
                            for x in 0..w {
                                let sx = ((f64::from(x) + 0.5) * 5. / f64::from(w)).floor().min(4.)
                                    as usize;
                                let sy = ((f64::from(y) + 0.5) * 3. / f64::from(h)).floor().min(2.)
                                    as usize;
                                assert_eq!(
                                    out.layers[layer].pixels[(y * w + x) as usize],
                                    d.layers[layer].pixels[sy * 5 + sx]
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    let mut edge = Document::new(2, 1, [255, 0, 0, 0]).unwrap();
    Arc::make_mut(&mut edge.layers[0].pixels)[1] = [0, 0, 255, 255];
    let out = gpu
        .resize_document(&edge, 1, 1, Resampler::Fant, false)
        .unwrap();
    assert_eq!(out.layers[0].pixels[0], [0, 0, 255, 128]);
    edge.layers[0].pixels = Arc::new(vec![[0, 0, 0, 255], [255; 4]]);
    let normal = gpu
        .resize_document(&edge, 1, 1, Resampler::Fant, false)
        .unwrap();
    let linear = gpu
        .resize_document(&edge, 1, 1, Resampler::Fant, true)
        .unwrap();
    assert_eq!(normal.layers[0].pixels[0], [128, 128, 128, 255]);
    assert_eq!(linear.layers[0].pixels[0], [188, 188, 188, 255]);
    let mut editor = Editor::new(d.clone());
    let resized = gpu
        .resize_document(&d, 10, 6, Resampler::Cubic, true)
        .unwrap();
    editor.edit("Resize", |d| *d = resized);
    editor.undo();
    assert_eq!(editor.document, d);
    editor.redo();
    assert_eq!(editor.document.width, 10);
    assert!(
        gpu.resize_document(&d, 0, 5, Resampler::Cubic, true)
            .is_err()
    );
    assert!(
        gpu.resize_document(&d, 16384, 16384, Resampler::Cubic, true)
            .is_err()
    );
}

#[test]
#[ignore = "Requires an explicitly verified hardware adapter"]
fn hardware_selection_outline_follows_mask_without_changing_document_pixels() {
    use eframe::wgpu;
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    assert_ne!(gpu.info.device_type, wgpu::DeviceType::Cpu);
    let mut d = Document::new(7, 7, [70, 100, 130, 255]).unwrap();
    let mut mask = vec![0; 49];
    for y in 1..6 {
        for x in 1..6 {
            mask[y * 7 + x] = 255;
        }
    }
    mask[3 * 7 + 3] = 0;
    d.selection = Some(Arc::new(mask));
    let canvas = gpu.compose_canvas(&d).unwrap();
    assert_eq!(
        gpu.read_pixels(&canvas.pixels).unwrap(),
        *d.layers[0].pixels
    );
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Selection display fixture"),
        size: 256 * 7,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &canvas.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(7),
            },
        },
        wgpu::Extent3d {
            width: 7,
            height: 7,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let pixels = gpu.read_pixels(&buffer).unwrap();
    for y in 0..7 {
        for x in 0..7 {
            let selected = d.selected(y * 7 + x);
            let neighbor = |x: i32, y: i32| {
                x >= 0 && y >= 0 && x < 7 && y < 7 && d.selected(y as usize * 7 + x as usize)
            };
            let edge = selected
                && (!neighbor(x as i32 - 1, y as i32)
                    || !neighbor(x as i32 + 1, y as i32)
                    || !neighbor(x as i32, y as i32 - 1)
                    || !neighbor(x as i32, y as i32 + 1));
            let expected = if edge {
                let stripe = if ((x + y) / 4) % 2 == 0 { 255 } else { 0 };
                [stripe, stripe, stripe, 255]
            } else {
                [70, 100, 130, 255]
            };
            assert_eq!(pixels[y * 64 + x], expected, "display pixel {x},{y}");
        }
    }
}
