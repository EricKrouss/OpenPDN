//! Direct2D's Robertson white point calculation, recovered from the native
//! reference's CalculateGains/TempTintToYUV/Yuv1960toXYZ routines.
mod temperature_table {
    include!("temperature_table.rs");
}
use temperature_table::LOCUS;

fn white_point(kelvin: f32, tint: f32) -> [f32; 3] {
    let reciprocal = 1. / kelvin.max(1666.6666);
    let i = (1..31)
        .find(|&i| reciprocal >= LOCUS[i - 1][3] && reciprocal <= LOCUS[i][3])
        .unwrap_or(30);
    let (a, b) = (LOCUS[i - 1], LOCUS[i]);
    let fraction = (reciprocal - a[3]) / (b[3] - a[3]);
    let normal_a = 1. / (1. + a[2] * a[2]).sqrt();
    let normal_b = 1. / (1. + b[2] * b[2]).sqrt();
    let normal = normal_a + (normal_b - normal_a) * fraction;
    let tangent_a = a[2] * normal_a;
    let slope = (tangent_a + (b[2] * normal_b - tangent_a) * fraction) / normal;
    let (base_u, base_v) = (
        a[0] + (b[0] - a[0]) * fraction,
        a[1] + (b[1] - a[1]) * fraction,
    );
    let displacement = -tint.clamp(-0.1, 0.1) / (1. + slope * slope).sqrt();
    let (mut u, mut v) = (base_u + displacement, base_v + displacement * slope);
    // Project chromaticities that cross the reference's gamut boundary back
    // along the tint vector, rather than independently clipping u and v.
    let boundary = 0.4 - u * 0.1;
    if v > boundary {
        let f = (boundary - v) / (base_v - (u - base_u) * 0.1 - v);
        u += (base_u - u) * f;
        v += (base_v - v) * f;
    }
    let x = 6. * u / (4. * v);
    let z = ((4. - u) * 6. - v * 60.) / (v * 12.);
    [
        x * 3.2404542 - 1.5371385 - z * 0.4985314,
        1.8760108 - x * 0.969266 + z * 0.041556,
        x * 0.0556434 - 0.2040259 + z * 1.0572252,
    ]
}

pub(super) fn gains(temperature: f32, tint: f32) -> [f32; 4] {
    let scaled = (temperature / 100.) * 100. * 0.012499999;
    let (reference_kelvin, target_kelvin) = if scaled > 0. {
        (6502.078, 1. / (scaled * 0.000_068_425_245 + 0.00015379698))
    } else if scaled < 0. {
        (
            1. / (0.00015379698 - scaled * 0.000084298255),
            1. / (0.00015379698 - scaled * -0.000053796983),
        )
    } else {
        (6502.078, 6502.078)
    };
    let a = white_point(reference_kelvin, 0.003256);
    let b = white_point(target_kelvin, 0.003256 + (tint / 100.) * 1.25 * 0.01);
    [
        (b[0] / b[1]) / (a[0] / a[1]),
        1.,
        (b[2] / b[1]) / (a[2] / a[1]),
        1.,
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_gain_fixtures() {
        let fixtures: Vec<[f32; 4]> =
            serde_json::from_str(include_str!("../../tests/fixtures/native-temperature.json"))
                .unwrap();
        for [temperature, tint, red, blue] in fixtures {
            let actual = super::gains(temperature, tint);
            assert!(
                (actual[0] - red).abs() < 0.00001 && (actual[2] - blue).abs() < 0.00001,
                "{temperature}/{tint}: {actual:?}, expected {red}/{blue}"
            );
        }
    }
}
