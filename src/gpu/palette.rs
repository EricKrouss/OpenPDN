//! Palette construction for Quantize. Octree means are computed in linear RGB.
//! The original uses WIC for Median Cut; our portable splitter is deterministic.
use crate::document::Pixel;
use std::collections::BTreeMap;
fn linear(v: u8) -> f64 {
    let s = f64::from(f32::from(v) / 255.);
    if s <= 0.04045 {
        f64::from((s * (1. / 12.92)) as f32)
    } else {
        f64::from(((s + 0.055) * (1. / 1.055)).powf(2.4) as f32)
    }
}
fn byte(v: f64) -> u8 {
    let v = f64::from(v as f32);
    let s = if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    };
    ((s as f32) * 255.).round().clamp(0., 255.) as u8
}
#[derive(Default)]
struct Node {
    children: [Option<usize>; 8],
    count: u64,
    sum: [f64; 3],
    depth: usize,
    parent: usize,
    slot: usize,
    active: bool,
}
impl Node {
    fn color(&self) -> Pixel {
        [
            byte(self.sum[0] / self.count as f64),
            byte(self.sum[1] / self.count as f64),
            byte(self.sum[2] / self.count as f64),
            255,
        ]
    }
}
fn octree(colors: &[(Pixel, u64)], limit: usize) -> Vec<Pixel> {
    let mut nodes = vec![Node {
        active: true,
        ..Default::default()
    }];
    for &(color, count) in colors {
        let mut at = 0;
        for depth in 0..8 {
            let bit = 7 - depth;
            let slot = usize::from(
                ((color[0] >> bit) & 1)
                    | (((color[1] >> bit) & 1) << 1)
                    | (((color[2] >> bit) & 1) << 2),
            );
            at = if let Some(child) = nodes[at].children[slot] {
                child
            } else {
                let child = nodes.len();
                nodes.push(Node {
                    depth: depth + 1,
                    parent: at,
                    slot,
                    active: true,
                    ..Default::default()
                });
                nodes[at].children[slot] = Some(child);
                child
            };
        }
        nodes[at].count += count;
        for (c, value) in color.iter().enumerate().take(3) {
            nodes[at].sum[c] += linear(*value) * count as f64;
        }
    }
    let mut color_count = colors.len();
    fn level(nodes: &[Node], at: usize, depth: usize, out: &mut Vec<usize>) {
        if nodes[at].depth == depth {
            out.push(at);
            return;
        }
        for child in nodes[at].children.iter().flatten() {
            level(nodes, *child, depth, out);
        }
    }
    fn merge(nodes: &mut [Node], index: usize, color_count: &mut usize) {
        let parent = nodes[index].parent;
        if nodes[parent].count == 0 {
            *color_count += 1;
        }
        nodes[parent].count += nodes[index].count;
        for c in 0..3 {
            nodes[parent].sum[c] += nodes[index].sum[c];
        }
        nodes[parent].children[nodes[index].slot] = None;
        nodes[index].active = false;
        *color_count -= 1;
    }
    // Merge COMPLETE levels in child traversal order until the parent level
    // can represent the target. Only then reduce the last level by population.
    let mut depth = 8;
    while depth >= 1 {
        let mut parents = Vec::new();
        level(&nodes, 0, depth - 1, &mut parents);
        if parents.len() <= limit {
            break;
        }
        let mut children = Vec::new();
        level(&nodes, 0, depth, &mut children);
        for index in children {
            merge(&mut nodes, index, &mut color_count);
        }
        depth -= 1;
    }
    let mut last = Vec::new();
    level(&nodes, 0, depth, &mut last);
    // Paint.NET uses process-randomized System.HashCode for equal populations.
    // A stable color order keeps repeat deterministic across native platforms.
    last.sort_by_key(|&i| (nodes[i].count, nodes[i].color()));
    for index in last {
        if color_count <= limit {
            break;
        }
        merge(&mut nodes, index, &mut color_count);
    }
    fn gather(nodes: &[Node], at: usize, out: &mut Vec<Pixel>) {
        let n = &nodes[at];
        if n.count > 0 {
            out.push(n.color());
        }
        for c in n.children.iter().flatten() {
            gather(nodes, *c, out);
        }
    }
    let mut result = Vec::new();
    gather(&nodes, 0, &mut result);
    result
}
fn median(colors: &[(Pixel, u64)], limit: usize) -> Vec<Pixel> {
    let mut boxes = vec![colors.to_vec()];
    while boxes.len() < limit {
        let chosen = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .max_by_key(|(_, b)| {
                let range = (0..3)
                    .map(|c| {
                        i32::from(b.iter().map(|v| v.0[c]).max().unwrap())
                            - i32::from(b.iter().map(|v| v.0[c]).min().unwrap())
                    })
                    .max()
                    .unwrap() as u64;
                range * b.iter().map(|v| v.1).sum::<u64>()
            })
            .map(|(i, _)| i);
        let Some(i) = chosen else {
            break;
        };
        let mut b = boxes.swap_remove(i);
        let channel = (0..3)
            .max_by_key(|&c| {
                b.iter().map(|v| v.0[c]).max().unwrap() - b.iter().map(|v| v.0[c]).min().unwrap()
            })
            .unwrap();
        b.sort_by_key(|v| v.0[channel]);
        let half = b.iter().map(|v| v.1).sum::<u64>() / 2;
        let mut weight = 0;
        let mut split = 1;
        for (i, (_, count)) in b.iter().enumerate() {
            weight += count;
            if weight >= half {
                split = (i + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let other = b.split_off(split);
        boxes.push(b);
        boxes.push(other);
    }
    boxes
        .iter()
        .map(|b| {
            let count = b.iter().map(|v| v.1).sum::<u64>() as f64;
            let mut color = [255; 4];
            for c in 0..3 {
                color[c] =
                    byte(b.iter().map(|(v, n)| linear(v[c]) * *n as f64).sum::<f64>() / count);
            }
            color
        })
        .collect()
}
pub(super) fn build(pixels: &[Pixel], values: [f32; 8]) -> Vec<[f32; 4]> {
    let has_transparent = pixels.iter().any(|v| v[3] == 0);
    let binary = pixels.iter().all(|v| v[3] == 0 || v[3] == 255);
    let mut transparent = values[3] != 0. && pixels.iter().any(|v| v[3] != 255);
    let limit = values[1] as usize - usize::from(transparent);
    let mut histogram = BTreeMap::<Pixel, u64>::new();
    for &pixel in pixels {
        if pixel[3] == 255 {
            *histogram.entry(pixel).or_default() += 1;
        }
    }
    let colors: Vec<_> = histogram.into_iter().collect();
    let lossless_count = colors.len().max(1) + usize::from(has_transparent);
    let mut palette = if binary && lossless_count <= values[1] as usize {
        transparent = has_transparent;
        let mut seen = std::collections::BTreeSet::new();
        let mut palette: Vec<_> = pixels
            .iter()
            .copied()
            .filter(|p| p[3] == 255 && seen.insert(*p))
            .collect();
        if palette.is_empty() {
            palette.push([0, 0, 0, 255]);
        }
        palette
    } else if colors.is_empty() {
        vec![[0, 0, 0, 255]]
    } else if values[0] == 1. {
        octree(&colors, limit)
    } else if colors.len() <= limit {
        colors.iter().map(|v| v.0).collect()
    } else {
        median(&colors, limit)
    };
    if transparent {
        palette.push([0; 4]);
    }
    proximity_kernel(palette)
}

// Original ListAlgorithms introsort: tie permutation is observable in proximity
// searches, which choose the last equally close palette color in this order.
fn proximity_sort(keys: &mut [usize], distances: &[i32]) {
    fn swap(keys: &mut [usize], d: &[i32], a: usize, b: usize) {
        if d[keys[a]] > d[keys[b]] {
            keys.swap(a, b);
        }
    }
    fn down(keys: &mut [usize], d: &[i32], mut i: usize, n: usize, lo: usize) {
        let v = keys[lo + i - 1];
        while i <= n / 2 {
            let mut child = 2 * i;
            if child < n && d[keys[lo + child - 1]] < d[keys[lo + child]] {
                child += 1;
            }
            if d[v] >= d[keys[lo + child - 1]] {
                break;
            }
            keys[lo + i - 1] = keys[lo + child - 1];
            i = child;
        }
        keys[lo + i - 1] = v;
    }
    fn sort(keys: &mut [usize], d: &[i32], lo: usize, mut hi: usize, mut depth: u32) {
        while hi > lo {
            let n = hi - lo + 1;
            if n <= 16 {
                if n == 2 {
                    swap(keys, d, lo, hi);
                } else if n == 3 {
                    swap(keys, d, lo, hi - 1);
                    swap(keys, d, lo, hi);
                    swap(keys, d, hi - 1, hi);
                } else {
                    for i in lo..hi {
                        let v = keys[i + 1];
                        let mut j = i + 1;
                        while j > lo && d[v] < d[keys[j - 1]] {
                            keys[j] = keys[j - 1];
                            j -= 1;
                        }
                        keys[j] = v;
                    }
                }
                break;
            }
            if depth == 0 {
                for i in (1..=n / 2).rev() {
                    down(keys, d, i, n, lo);
                }
                for i in (2..=n).rev() {
                    keys.swap(lo, lo + i - 1);
                    down(keys, d, 1, i - 1, lo);
                }
                break;
            }
            depth -= 1;
            let mid = lo + (hi - lo) / 2;
            swap(keys, d, lo, mid);
            swap(keys, d, lo, hi);
            swap(keys, d, mid, hi);
            let pivot = keys[mid];
            keys.swap(mid, hi - 1);
            let mut left = lo;
            let mut right = hi - 1;
            loop {
                left += 1;
                while d[keys[left]] < d[pivot] {
                    left += 1;
                }
                right -= 1;
                while d[pivot] < d[keys[right]] {
                    right -= 1;
                }
                if left >= right {
                    break;
                }
                keys.swap(left, right);
            }
            keys.swap(left, hi - 1);
            sort(keys, d, left + 1, hi, depth);
            hi = left - 1;
        }
    }
    if keys.len() > 1 {
        sort(
            keys,
            distances,
            0,
            keys.len() - 1,
            2 * (keys.len().ilog2() + 1),
        );
    }
}
fn proximity_kernel(palette: Vec<Pixel>) -> Vec<[f32; 4]> {
    let opaque = palette.iter().take_while(|p| p[3] == 255).count();
    let stride = opaque.div_ceil(4);
    let mut data = Vec::with_capacity(1 + palette.len() + 4096 * stride);
    data.push([palette.len() as f32, opaque as f32, stride as f32, 0.]);
    data.extend(palette.iter().map(|c| c.map(f32::from)));
    for cube in 0..4096 {
        let center = [
            (cube & 15) * 16 + 8,
            ((cube >> 4) & 15) * 16 + 8,
            (cube >> 8) * 16 + 8,
        ];
        let distances: Vec<_> = palette[..opaque]
            .iter()
            .map(|c| (0..3).map(|i| (i32::from(c[i]) - center[i]).pow(2)).sum())
            .collect();
        let mut indices: Vec<_> = (0..opaque).collect();
        proximity_sort(&mut indices, &distances);
        for group in indices.chunks(4) {
            let mut row = [0.; 4];
            for (dst, src) in row.iter_mut().zip(group) {
                *dst = *src as f32;
            }
            data.push(row);
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_octree_palettes_unique_populations() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/paintnet-5.1.12-octree.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let entries: Vec<_> = case["entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    let color = std::array::from_fn(|c| v["color"][c].as_u64().unwrap() as u8);
                    (color, v["count"].as_u64().unwrap())
                })
                .collect();
            let expected: Vec<Pixel> = case["palette"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| std::array::from_fn(|c| v[c].as_u64().unwrap() as u8))
                .collect();
            assert_eq!(
                octree(&entries, case["limit"].as_u64().unwrap() as usize),
                expected,
                "seed {} limit {}",
                case["seed"],
                case["limit"]
            );
        }
    }
    #[test]
    fn original_proximity_queries_and_ties() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/paintnet-5.1.12-proximity.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let palette: Vec<Pixel> = case["palette"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| std::array::from_fn(|c| v[c].as_u64().unwrap() as u8))
                .collect();
            let kernel = proximity_kernel(palette.clone());
            let stride = palette.len().div_ceil(4);
            for target in case["targets"].as_array().unwrap() {
                let color: [i32; 3] =
                    std::array::from_fn(|c| target["color"][c].as_i64().unwrap() as i32);
                let cube = (color[0] >> 4) | ((color[1] >> 4) << 4) | ((color[2] >> 4) << 8);
                let mut best = 0;
                let mut distance = i32::MAX;
                for j in 0..palette.len() {
                    let k =
                        kernel[1 + palette.len() + cube as usize * stride + j / 4][j % 4] as usize;
                    let d = (0..3)
                        .map(|c| (i32::from(palette[k][c]) - color[c]).pow(2))
                        .sum();
                    if d <= distance {
                        distance = d;
                        best = k;
                    }
                }
                assert_eq!(
                    best,
                    target["index"].as_u64().unwrap() as usize,
                    "palette {} color {color:?}",
                    palette.len()
                );
            }
        }
    }
}
