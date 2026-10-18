//! The original Add Noise inverse distribution table. Construct once in
//! double precision; noise sampling and image processing run on the GPU.
use std::sync::OnceLock;

pub(super) fn lookup_table() -> &'static [i32; 16384] {
    static TABLE: OnceLock<[i32; 16384]> = OnceLock::new();
    TABLE.get_or_init(|| {
        const N: usize = 16384;
        let curve = |i: usize, scale: f64| {
            let x = 16. * (i as f64 - (N / 2) as f64) / N as f64;
            scale * (-x * x / 2.).exp()
        };
        let (mut lo, mut hi, mut scale) = (5., 10., 50.);
        while hi - lo > 1e-7 {
            scale = (lo + hi) * 0.5;
            let mut total = 0.;
            for i in 0..N {
                total += curve(i, scale);
                if total > 1_000_000. {
                    break;
                }
            }
            if total > N as f64 {
                hi = scale;
            } else if total < N as f64 {
                lo = scale;
            } else {
                break;
            }
        }
        let mut table = [0; N];
        let mut total = 0.;
        let mut begin = 0;
        for i in 0..N {
            total += curve(i, scale);
            let end = total as usize;
            table[begin..end].fill((i as i32 - (N / 2) as i32) * 65536 / N as i32);
            begin = end;
        }
        table
    })
}

pub(super) fn pcg(input: u32) -> u32 {
    let state = input.wrapping_mul(747796405).wrapping_add(2891336453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277803737);
    (word >> 22) ^ word
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_managed_noise_table_and_pcg() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/paintnet-5.1.12-noise.json"
        ))
        .unwrap();
        let expected: Vec<i32> = fixture["lookup_table"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        assert_eq!(lookup_table().as_slice(), expected);
        for case in fixture["hashes"].as_array().unwrap() {
            let hash = pcg(case["input"].as_u64().unwrap() as u32);
            assert_eq!(u64::from(hash), case["output"].as_u64().unwrap());
            assert_eq!(u64::from(hash), case["next_seed"].as_u64().unwrap());
            let next = (hash >> 8) as f32 / 16777216.;
            assert_eq!(next, case["next_float"].as_f64().unwrap() as f32);
        }
    }
}
