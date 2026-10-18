//! ColorWheelControl's integer gestures and procedural sRGB wheel pixels.
use crate::document::Pixel;
use std::f64::consts::PI;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WheelModifiers {
    pub snap: bool,
    pub lock_hue: bool,
    pub lock_radius: bool,
}
pub fn grab_color(
    delta: [f64; 2],
    radius: f64,
    basis: [u16; 3],
    modifiers: WheelModifiers,
) -> [u16; 3] {
    let [x, y] = delta;
    let mut angle = y.atan2(x);
    if angle < 0. {
        angle += PI * 2.;
    }
    if modifiers.snap {
        let value = angle * (180. / PI) / 15.;
        let lo = value.floor();
        let hi = value.ceil();
        let closest = if (value - hi).abs() < (value - lo).abs() {
            hi
        } else {
            lo
        };
        angle = closest * 15. * (PI / 180.);
    }
    let distance = if modifiers.lock_hue {
        let hue = f64::from(basis[0]) * (PI / 180.);
        (x * (-hue).cos() - y * (-hue).sin()).max(0.)
    } else {
        (x * x + y * y).sqrt()
    };
    let mut result = basis;
    if !modifiers.lock_hue {
        result[0] = (0.5 + angle * (180. / PI)) as u16;
    }
    if !modifiers.lock_radius {
        result[1] = (0.5 + 100. * (distance / radius).min(1.).powf(1.4)) as u16;
    }
    if result[2] == 0 {
        result[2] = 100;
    }
    result
}
pub fn wheel_pixel(delta: [f64; 2], radius: f64) -> Pixel {
    if radius <= 0. {
        return [0; 4];
    }
    let [x, y] = delta;
    let distance = (x * x + y * y).sqrt();
    if distance >= radius + 1. {
        return [0; 4];
    }
    let mut angle = if x == 0. && y == 0. { 0. } else { y.atan2(x) };
    if angle < 0. {
        angle += PI * 2.;
    }
    let hue = angle * 3. / PI;
    let (s, a) = if distance <= radius {
        ((distance / radius).powf(1.4), 1.)
    } else {
        (1., 1. - (distance - radius))
    };
    if s == 0. {
        return [255; 4];
    }
    let sector = hue as u32;
    let f = hue - f64::from(sector);
    let p = 255. * (1. - s);
    let q = 255. * (1. - s * f);
    let t = 255. * (1. - s * (1. - f));
    let [r, g, b] = match sector {
        0 => [255., t, p],
        1 => [q, 255., p],
        2 => [p, 255., t],
        3 => [p, q, 255.],
        4 => [t, p, 255.],
        5 => [255., p, q],
        _ => [0.; 3],
    };
    [
        (r + 0.5) as u8,
        (g + 0.5) as u8,
        (b + 0.5) as u8,
        (a * 255. + 0.5) as u8,
    ]
}
