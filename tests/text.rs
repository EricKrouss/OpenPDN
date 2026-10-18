use openpdn::{content::*, text::*};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OriginalCase {
    lines: Vec<String>,
    line: usize,
    offset: usize,
    operation: String,
    result: Vec<String>,
    result_line: usize,
    result_offset: usize,
}
#[test]
fn original_text_navigation_and_deletion() {
    let cases: Vec<OriginalCase> =
        serde_json::from_str(include_str!("fixtures/paintnet-5.1.12-text.json")).unwrap();
    assert_eq!(cases.len(), 512);
    for case in cases {
        let mut text = CanvasText::new(case.lines.join("\n")).unwrap();
        let offset = case.lines[case.line]
            .char_indices()
            .nth(case.offset)
            .map_or(case.lines[case.line].len(), |(i, _)| i);
        text.set_cursor(case.line, offset);
        match case.operation.as_str() {
            "PerformLeft" => text.motion(TextMotion::Left),
            "PerformRight" => text.motion(TextMotion::Right),
            "PerformControlLeft" => text.motion(TextMotion::WordLeft),
            "PerformControlRight" => text.motion(TextMotion::WordRight),
            "PerformControlBackspace" => text.delete(true, true),
            "PerformControlDelete" => text.delete(false, true),
            _ => unreachable!(),
        }
        assert_eq!(
            text.text(),
            case.result.join("\n"),
            "{} at {:?}",
            case.operation,
            (case.line, case.offset)
        );
        let (line, offset) = text.line_cursor();
        let chars = text.text().split('\n').nth(line).unwrap()[..offset]
            .chars()
            .count();
        assert_eq!(
            (line, chars),
            (case.result_line, case.result_offset),
            "{} {:?}",
            case.operation,
            case.lines
        );
    }
}
#[test]
fn composition_unicode_and_text_limits() {
    let mut text = CanvasText::default();
    text.insert("first\r\nsecond\t");
    assert_eq!(text.text(), "first\nsecond");
    text.preedit("候補", Some(0..1));
    assert_eq!(text.text(), "first\nsecond候補");
    text.preedit("候補者", Some(1..3));
    assert_eq!(text.text(), "first\nsecond候補者");
    text.commit("確定🦀");
    assert_eq!(text.text(), "first\nsecond確定🦀");
    text.delete(true, false);
    assert_eq!(text.text(), "first\nsecond確定");
    text.preedit("取消", None);
    text.cancel_composition();
    assert_eq!(text.text(), "first\nsecond確定");
    text.delete_surrounding(1, usize::MAX);
    assert_eq!(text.text(), "first\nsecond確");
    text.motion(TextMotion::Home);
    text.delete(true, false);
    assert_eq!(text.text(), "firstsecond確");
    text.insert(&"x".repeat(65536));
    assert_eq!(
        text.text(),
        "firstsecond確",
        "Oversize input must leave valid text intact"
    );
    assert!(CanvasText::new("x".repeat(65537)).is_err());
}
#[test]
fn caret_layout_alignment_empty_lines_and_clicks() {
    let mut renderer = TextRenderer::default();
    for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
        let layout = renderer
            .layout(
                "café\n\nRust ",
                &TextOptions {
                    align,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(layout.lines.len(), 3);
        for (line, run) in layout.lines.iter().enumerate() {
            for &(i, x) in &run.cursors {
                let (cx, y, h) = layout.caret(line, i);
                assert!((cx - x).abs() < 1e-5 && h > 0.);
                assert_eq!(layout.hit(x, y + h / 4.), (line, i));
            }
            let (end, x) = *run.cursors.last().unwrap();
            assert_eq!(layout.hit(x + 100., run.top), (line, end));
        }
        let empty = renderer
            .layout(
                "",
                &TextOptions {
                    align,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(empty.hit(-500., -200.), (0, 0));
        assert!(empty.caret(0, 0).2 > 0.);
    }
}
