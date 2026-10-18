//! Recovered Microsoft GDI+ 10.0.19041.329 RedEyeCorrection analysis.
//! Selection components define the native effect's areas. Polar reference
//! statistics are small GPU parameters; correction of the image stays on GPU.
use crate::document::{Document, Pixel};
// GDI+ executes this analysis with MXCSR=0x3fa0 (round down).
// Preserve that arithmetic portably without changing the caller's FP state.
fn down(value: f64) -> f32 {
    let nearest = value as f32;
    if f64::from(nearest) > value {
        nearest.next_down()
    } else {
        nearest
    }
}
fn add(a: f32, b: f32) -> f32 {
    down(f64::from(a) + f64::from(b))
}
fn sub(a: f32, b: f32) -> f32 {
    down(f64::from(a) - f64::from(b))
}
fn mul(a: f32, b: f32) -> f32 {
    down(f64::from(a) * f64::from(b))
}
fn div(a: f32, b: f32) -> f32 {
    down(f64::from(a) / f64::from(b))
}
fn squared(x: f32, y: f32) -> f32 {
    add(mul(y, y), mul(x, x))
}
fn sector(x: f32, y: f32) -> usize {
    let angle = if x == 0. {
        if y < 0. { 270. } else { 90. }
    } else {
        let a = (f64::from(y) / f64::from(x)).atan() * 180. / std::f64::consts::PI;
        if y < 0. {
            if a < 0. { a + 360. } else { a + 180. }
        } else if a >= 0. {
            a
        } else {
            a + 180.
        }
    };
    (div(down(angle), 6.).floor() as usize).min(59)
}
fn luminance(c: Pixel) -> f32 {
    (f32::from(c[1]) * 9. + f32::from(c[0]) * 5. + f32::from(c[2]) * 2.) / 16.
}
fn complement(c: Pixel) -> f32 {
    div(f32::from(c[2]) * 2. + f32::from(c[1]) * 9., 11.)
}
fn excess(c: Pixel) -> f32 {
    (f32::from(c[0]) - f32::from(c[1].max(c[2]))).max(0.)
}
fn areas(doc: &Document) -> Vec<[u32; 4]> {
    if doc.selection.is_none() {
        return vec![[0, 0, doc.width, doc.height]];
    }
    let mut seen = vec![false; (doc.width * doc.height) as usize];
    let mut result = Vec::new();
    for i in 0..seen.len() {
        if seen[i] || !doc.selected(i) {
            continue;
        }
        seen[i] = true;
        let mut queue = vec![i];
        let mut cursor = 0;
        let mut bounds = [
            i as u32 % doc.width,
            i as u32 / doc.width,
            i as u32 % doc.width + 1,
            i as u32 / doc.width + 1,
        ];
        while cursor < queue.len() {
            let at = queue[cursor];
            cursor += 1;
            let x = at as u32 % doc.width;
            let y = at as u32 / doc.width;
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x + 1);
            bounds[3] = bounds[3].max(y + 1);
            for (dx, dy) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= doc.width as i32 || ny >= doc.height as i32 {
                    continue;
                }
                let neighbor = ny as usize * doc.width as usize + nx as usize;
                if !seen[neighbor] && doc.selected(neighbor) {
                    seen[neighbor] = true;
                    queue.push(neighbor);
                }
            }
        }
        result.push(bounds);
    }
    result
}
#[derive(Default)]
pub(super) struct State {
    red_center: [f32; 2],
    dark_center: [f32; 2],
    max_luma: usize,
    max_point: [f32; 2],
}
pub(super) fn parameters(doc: &Document, state: &mut State) -> Vec<[f32; 4]> {
    let pixels = &doc.layers[doc.active].pixels;
    let mut table = Vec::new();
    for bounds in areas(doc) {
        let [left, top, right, bottom] = bounds;
        let geometric = [(left + right) as f32 * 0.5, (top + bottom) as f32 * 0.5];
        let scan = || {
            (top..bottom).flat_map(move |y| {
                (left..right)
                    .map(move |x| (x as f32, y as f32, pixels[(y * doc.width + x) as usize]))
            })
        };
        let mut center_sums = [0_u32; 4];
        for (x, y, c) in scan() {
            if c[3] != 0 && squared(sub(x, geometric[0]), sub(y, geometric[1])) < 121. {
                for channel in 0..3 {
                    center_sums[channel] += u32::from(c[channel]);
                }
                center_sums[3] += 1;
            }
        }
        let averages = center_sums.map(|v| div(v as f32, center_sums[3].max(1) as f32));
        let auto = averages[1] > 150.
            && averages[2] > 150.
            && averages[0] < averages[1] + 10.
            && averages[0] < averages[2] + 10.;
        let mut histogram = [0_u32; 256];
        let mut red_center = state.red_center;
        let mut dark_center = state.dark_center;
        let mut red_weight = 0.;
        let mut dark_weight = 0_u32;
        let mut max_luma = state.max_luma;
        let mut max_point = state.max_point;
        let mut count = 0_u32;
        for (x, y, c) in scan() {
            if c[3] == 0 {
                continue;
            }
            let comp = add(complement(c), 0.5).floor() as usize;
            let e = excess(c);
            let weight = if e <= 0. {
                e
            } else if comp == 0 {
                1.
            } else {
                div(e, comp as f32)
            };
            histogram[comp] += 1;
            red_weight = add(red_weight, weight);
            let dark = 255 - comp as u32;
            dark_weight += dark;
            count += 1;
            red_center[0] = add(red_center[0], mul(x, weight));
            red_center[1] = add(red_center[1], mul(y, weight));
            dark_center[0] = add(dark_center[0], down(f64::from(x) * f64::from(dark)));
            dark_center[1] = add(dark_center[1], down(f64::from(y) * f64::from(dark)));
            if comp > max_luma {
                max_luma = comp;
                max_point = [x, y];
            }
        }
        if red_weight != 0. {
            red_center = red_center.map(|v| div(v, red_weight));
        }
        if dark_weight != 0 {
            dark_center = dark_center.map(|v| div(v, dark_weight as f32));
        }
        // GDI+ retains these accumulators between areas and strength passes.
        state.red_center = red_center;
        state.dark_center = dark_center;
        state.max_luma = max_luma;
        state.max_point = max_point;
        let mut low = 0;
        let mut high = 0;
        let mut cumulative = 0_u32;
        for (i, n) in histogram.iter().enumerate() {
            cumulative += n;
            let f = f64::from(div(cumulative as f32, count.max(1) as f32));
            if f < 0.3 {
                low = i;
            }
            if f < 0.7 {
                high = i;
            }
        }
        let distance = squared(
            sub(dark_center[0], red_center[0]),
            sub(dark_center[1], red_center[1]),
        );
        let scale = div(
            mul(
                (right - left + bottom - top) as f32 * 0.5,
                (right - left + bottom - top) as f32 * 0.5,
            ),
            9.,
        );
        let mut centered = distance < scale;
        let mut center = if centered { red_center } else { dark_center };
        if auto {
            center = geometric;
            centered = false;
        }
        let mut radius = center[0].min(center[1]);
        if center[0] <= center[1] {
            if center[1] <= ((top + bottom) / 2) as f32 {
                radius = radius.min(center[1] - top as f32);
            } else {
                radius = radius.min(bottom as f32 - center[1]);
            }
        } else if center[0] <= ((left + right) / 2) as f32 {
            radius = radius.min(center[0] - left as f32);
        } else {
            radius = radius.min(right as f32 - center[0]);
        }
        let mut min_luma = [u32::MAX; 60];
        let mut distances = [0.; 60];
        let mut average_excess = [0.; 60];
        let mut average_complement = [0.; 60];
        let mut reference = [0.; 60];
        let mut populations = [0_u32; 60];
        for (x, y, c) in scan() {
            if c[3] == 0 {
                continue;
            }
            let dx = sub(x, center[0]);
            let dy = sub(y, center[1]);
            let squared = squared(dx, dy);
            let bin = sector(dx, dy);
            let lum = (luminance(c) + 0.5).floor() as u32;
            if lum < min_luma[bin] && squared >= 121. {
                min_luma[bin] = lum;
                reference[bin] = lum as f32;
                distances[bin] = down(f64::from(squared).sqrt());
            }
        }
        if auto {
            let original_radius = radius;
            radius = div(distances.iter().fold(0., |a, b| add(a, *b)), 60.).min(original_radius);
            for i in 0..60 {
                if distances[i] == 0. {
                    distances[i] = 11.;
                }
                if radius <= distances[i] {
                    reference[i] = 0.;
                }
            }
        } else {
            distances.fill(radius);
        }
        for (x, y, c) in scan() {
            if c[3] == 0 {
                continue;
            }
            let dx = sub(x, center[0]);
            let dy = sub(y, center[1]);
            if squared(dx, dy) < mul(radius, radius) {
                let bin = sector(dx, dy);
                populations[bin] += 1;
                average_excess[bin] = add(average_excess[bin], excess(c));
                average_complement[bin] = add(average_complement[bin], complement(c));
                if radius <= distances[bin] {
                    reference[bin] = add(reference[bin], luminance(c));
                }
            }
        }
        let mut factor = 1.;
        if centered {
            let range = high - low;
            factor = if range >= 81 {
                -0.75
            } else if range >= 41 {
                -0.66
            } else if range >= 26 {
                -0.5
            } else {
                -0.25
            };
        }
        table.push(bounds.map(|v| v as f32));
        table.push([center[0], center[1], radius, f32::from(auto)]);
        table.push([max_point[0], max_point[1], f32::from(centered), factor]);
        table.push([max_luma as f32, high as f32, geometric[0], geometric[1]]);
        for i in 0..60 {
            let n = populations[i] as f32;
            if n == 0. {
                average_excess[i] = 0.;
                average_complement[i] = 1.;
            } else {
                average_excess[i] = div(average_excess[i], n);
                average_complement[i] = div(average_complement[i], n);
                reference[i] = div(reference[i], n).max(1.);
            }
            table.push([average_excess[i], average_complement[i], reference[i], 0.]);
        }
    }
    if table.is_empty() {
        table.resize(64, [0.; 4]);
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_analysis() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/native-redeye.json")).unwrap();
        let expected: Vec<Vec<[f32; 4]>> = serde_json::from_str(include_str!(
            "../../tests/fixtures/native-redeye-analysis.json"
        ))
        .unwrap();
        let mut state = State::default();
        for (i, case) in cases.as_array().unwrap().iter().enumerate() {
            if i % 5 == 0 {
                state = State::default();
            }
            let mut doc = Document::new(41, 33, [0; 4]).unwrap();
            let pixels: Vec<Pixel> = serde_json::from_value(if i % 5 == 0 {
                case["input"].clone()
            } else {
                cases[i - 1]["output"].clone()
            })
            .unwrap();
            doc.layers[0].pixels = std::sync::Arc::new(pixels);
            let actual = parameters(&doc, &mut state);
            for (j, (a, b)) in actual.iter().zip(&expected[i]).enumerate() {
                for k in 0..4 {
                    assert!(
                        (a[k] - b[k]).abs() < 0.0002,
                        "case {i} entry {j} channel {k}: {a:?} != {b:?}"
                    );
                }
            }
        }
    }
}
