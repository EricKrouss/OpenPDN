//! Hand-drawn vector artwork. The same paths drive the UI and the reviewable SVG atlas.
//! No generated bitmaps, icon fonts, copied resource files, or generic fallback glyphs.
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use std::fmt::Write as _;
macro_rules! icons {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Icon { $($variant),+ }
        impl Icon {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub fn name(self) -> &'static str { match self { $(Self::$variant => $name),+ } }
            pub fn named(name: &str) -> Option<Self> { match name { $($name => Some(Self::$variant)),+, _ => None } }
        }
    };
}
icons! {
    RectangleSelect => "rect", MovePixels => "move", Lasso => "lasso", MoveSelection => "move-selection",
    EllipseSelect => "ellipse", Zoom => "zoom", MagicWand => "wand", Pan => "pan", Bucket => "bucket",
    Gradient => "gradient", Brush => "brush", Eraser => "eraser", Pencil => "pencil", Picker => "picker",
    ToolBlend => "tool-blend", HatchFill => "hatch-fill",
    SwapColors => "swap-colors", ResetColors => "reset-colors", Palette => "palette", AddColor => "add-color",
    SelectionRatio => "selection-ratio", SelectionSize => "selection-size",
    BrushCircle => "brush-circle", BrushSquare => "brush-square", BrushHardness => "brush-hardness",
    BrushSpacing => "brush-spacing", BrushAntialias => "brush-antialias", BrushSmoothing => "brush-smoothing", BrushPressure => "brush-pressure",
    CloneStamp => "stamp", Recolor => "recolor", Text => "text", Line => "line", Shapes => "shapes",
    DragDrop => "drag-drop", New => "new", Open => "open", Save => "save", Print => "print", Cut => "cut",
    Copy => "copy", Paste => "paste", Crop => "crop", Deselect => "deselect", Undo => "undo", Redo => "redo",
    Grid => "grid", Rulers => "rulers", Tools => "tools", History => "history", Layers => "layers", Colors => "colors",
    FloatWindow => "float-window", DockWindow => "dock-window",
    Add => "add", Delete => "delete", Duplicate => "duplicate", Up => "up", Down => "down", Merge => "merge",
    Properties => "properties", Close => "close", FlipHorizontal => "flip-horizontal", FlipVertical => "flip-vertical",
    Rotate => "rotate", Resize => "resize", CanvasSize => "canvas-size", Flatten => "flatten",
    AutoLevel => "auto-level", BlackWhite => "black-white", BrightnessContrast => "brightness-contrast", Curves => "curves",
    Exposure => "exposure", HighlightsShadows => "highlights-shadows", HueSaturation => "hue-saturation",
    InvertAlpha => "invert-alpha", InvertColors => "invert-colors", Levels => "levels", Posterize => "posterize",
    Sepia => "sepia", TemperatureTint => "temperature-tint",
    InkSketch => "ink-sketch", OilPainting => "oil-painting", PencilSketch => "pencil-sketch",
    Bokeh => "bokeh", Fragment => "fragment", GaussianBlur => "gaussian-blur", Median => "median", MotionBlur => "motion-blur",
    RadialBlur => "radial-blur", SketchBlur => "sketch-blur", SquareBlur => "square-blur", SurfaceBlur => "surface-blur", ZoomBlur => "zoom-blur",
    Quantize => "quantize", Bulge => "bulge", Crystalize => "crystalize", Dents => "dents", FrostedGlass => "frosted-glass",
    Morphology => "morphology", Pixelate => "pixelate", PolarInversion => "polar-inversion", TileReflection => "tile-reflection", Twist => "twist",
    AddNoise => "add-noise", ReduceNoise => "reduce-noise", DropShadow => "drop-shadow", Glow => "glow", RedEye => "red-eye",
    Sharpen => "sharpen", SoftenPortrait => "soften-portrait", Straighten => "straighten", Vignette => "vignette",
    Clouds => "clouds", Julia => "julia", Mandelbrot => "mandelbrot", Turbulence => "turbulence",
    EdgeDetect => "edge-detect", Emboss => "emboss", Outline => "outline", Relief => "relief"
}
#[derive(Clone, Copy, PartialEq)]
enum Ink {
    Dark,
    Blue,
    Pale,
    Gold,
    Green,
    Red,
    Purple,
    White,
}
impl Ink {
    fn rgb(self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgb(57, 65, 73),
            Self::Blue => Color32::from_rgb(48, 122, 181),
            Self::Pale => Color32::from_rgb(170, 207, 230),
            Self::Gold => Color32::from_rgb(224, 166, 48),
            Self::Green => Color32::from_rgb(62, 154, 97),
            Self::Red => Color32::from_rgb(207, 82, 82),
            Self::Purple => Color32::from_rgb(151, 94, 177),
            Self::White => Color32::from_rgb(250, 251, 252),
        }
    }
}
struct Mark {
    points: Vec<[f32; 2]>,
    closed: bool,
    fill: Option<Ink>,
    stroke: Option<Ink>,
}
#[derive(Default)]
struct Drawing(Vec<Mark>);
impl Drawing {
    fn path(&mut self, points: &[[f32; 2]], color: Ink) {
        self.0.push(Mark {
            points: points.into(),
            closed: false,
            fill: None,
            stroke: Some(color),
        });
    }
    fn polygon(&mut self, points: &[[f32; 2]], fill: Ink) {
        self.0.push(Mark {
            points: points.into(),
            closed: true,
            fill: Some(fill),
            stroke: Some(Ink::Dark),
        });
    }
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, fill: Option<Ink>, stroke: Ink) {
        self.0.push(Mark {
            points: vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
            closed: true,
            fill,
            stroke: Some(stroke),
        });
    }
    fn oval(&mut self, x: f32, y: f32, rx: f32, ry: f32, fill: Option<Ink>, stroke: Ink) {
        let points = (0..32)
            .map(|n| {
                let t = n as f32 * std::f32::consts::TAU / 32.;
                [x + rx * t.cos(), y + ry * t.sin()]
            })
            .collect();
        self.0.push(Mark {
            points,
            closed: true,
            fill,
            stroke: Some(stroke),
        });
    }
    fn dash_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        for n in 0..4 {
            let a = n as f32 / 4.;
            let b = a + 0.14;
            self.path(&[[x + w * a, y], [x + w * b, y]], Ink::Dark);
            self.path(&[[x + w * a, y + h], [x + w * b, y + h]], Ink::Dark);
            self.path(&[[x, y + h * a], [x, y + h * b]], Ink::Dark);
            self.path(&[[x + w, y + h * a], [x + w, y + h * b]], Ink::Dark);
        }
    }
    fn arrow(&mut self, a: [f32; 2], b: [f32; 2], color: Ink) {
        self.path(&[a, b], color);
        let v = [a[0] - b[0], a[1] - b[1]];
        let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
        let v = [v[0] / len * 4., v[1] / len * 4.];
        self.path(
            &[
                [b[0] + v[0] - v[1] * 0.7, b[1] + v[1] + v[0] * 0.7],
                b,
                [b[0] + v[0] + v[1] * 0.7, b[1] + v[1] - v[0] * 0.7],
            ],
            color,
        );
    }
    fn star(&mut self, x: f32, y: f32, r: f32, color: Ink) {
        self.path(&[[x - r, y], [x + r, y]], color);
        self.path(&[[x, y - r], [x, y + r]], color);
    }
    fn page(&mut self, x: f32, y: f32) {
        self.polygon(
            &[
                [x, y],
                [x + 9., y],
                [x + 13., y + 4.],
                [x + 13., y + 17.],
                [x, y + 17.],
            ],
            Ink::White,
        );
        self.path(
            &[[x + 9., y], [x + 9., y + 4.], [x + 13., y + 4.]],
            Ink::Blue,
        );
    }
    fn picture(&mut self) {
        self.rect(3., 4., 18., 16., Some(Ink::White), Ink::Blue);
        self.path(
            &[[4., 18.], [9., 11.], [13., 15.], [16., 10.], [20., 18.]],
            Ink::Blue,
        );
        self.oval(8., 8., 1.5, 1.5, Some(Ink::Gold), Ink::Gold);
    }
    fn wave(&mut self, y: f32, phase: f32, color: Ink) {
        let points: Vec<_> = (0..19)
            .map(|n| [3. + n as f32, y + ((n as f32 + phase) * 0.6).sin() * 2.5])
            .collect();
        self.path(&points, color);
    }
}
fn artwork(icon: Icon) -> Drawing {
    use Icon::*;
    use Ink::*;
    let mut d = Drawing::default();
    match icon {
        FloatWindow | DockWindow => {
            d.rect(2., 7., 15., 14., Some(White), Dark);
            d.path(&[[3., 10.], [16., 10.]], Blue);
            d.rect(12., 2., 10., 8., Some(Pale), Blue);
            if icon == FloatWindow {
                d.arrow([7., 16.], [18., 5.], Blue);
            } else {
                d.arrow([18., 5.], [7., 16.], Green);
            }
        }
        SwapColors => {
            d.rect(2., 3., 7., 7., Some(Blue), Dark);
            d.rect(15., 14., 7., 7., Some(Gold), Dark);
            d.arrow([11., 5.], [21., 5.], Blue);
            d.arrow([13., 19.], [3., 19.], Dark);
        }
        ResetColors => {
            d.rect(3., 3., 12., 12., Some(Dark), Dark);
            d.rect(9., 9., 12., 12., Some(White), Dark);
        }
        Palette | AddColor => {
            d.rect(2., 3., 20., 18., Some(White), Dark);
            for (n, c) in [Blue, Gold, Green, Dark, Pale, White]
                .into_iter()
                .enumerate()
            {
                d.rect(
                    4. + (n % 3) as f32 * 6.,
                    5. + (n / 3) as f32 * 7.,
                    4.,
                    5.,
                    Some(c),
                    Dark,
                );
            }
            if icon == AddColor {
                d.path(&[[17., 14.], [17., 22.]], Blue);
                d.path(&[[13., 18.], [21., 18.]], Blue);
            }
        }
        SelectionRatio | SelectionSize => {
            d.dash_rect(4., 4., 16., 14.);
            if icon == SelectionRatio {
                d.path(&[[7., 15.], [17., 7.]], Blue);
            } else {
                d.arrow([3., 21.], [21., 21.], Blue);
                d.arrow([1., 18.], [1., 3.], Dark);
            }
        }
        ToolBlend => {
            d.rect(3., 3., 12., 12., Some(Pale), Blue);
            d.rect(9., 9., 12., 12., Some(Gold), Dark);
            d.rect(9., 9., 6., 6., Some(Green), Green);
        }
        HatchFill => {
            d.rect(3., 3., 18., 18., Some(White), Blue);
            for offset in [0., 5., 10., 15.] {
                d.path(&[[3. + offset, 21.], [21., 3. + offset]], Dark);
                if offset > 0. {
                    d.path(&[[3., 21. - offset], [21. - offset, 3.]], Dark);
                }
            }
        }
        RectangleSelect => d.dash_rect(3., 4., 18., 16.),
        MovePixels | MoveSelection => {
            if icon == MoveSelection {
                d.dash_rect(2., 3., 13., 12.);
            } else {
                d.rect(2., 3., 12., 11., Some(Pale), Blue);
                d.rect(4., 5., 3., 3., Some(White), White);
            }
            d.polygon(
                &[
                    [10., 8.],
                    [12., 22.],
                    [15., 18.],
                    [19., 22.],
                    [21., 20.],
                    [17., 16.],
                    [22., 14.],
                ],
                White,
            );
        }
        Lasso => {
            d.path(
                &[
                    [16., 16.],
                    [19., 13.],
                    [20., 8.],
                    [17., 4.],
                    [10., 3.],
                    [5., 5.],
                    [3., 10.],
                    [6., 15.],
                    [12., 17.],
                    [16., 16.],
                    [17., 20.],
                    [14., 22.],
                    [12., 20.],
                    [13., 17.],
                ],
                Dark,
            );
        }
        EllipseSelect => {
            for n in 0..12 {
                let t = n as f32 * std::f32::consts::TAU / 12.;
                d.path(
                    &[
                        [12. + 9. * t.cos(), 12. + 7. * t.sin()],
                        [12. + 9. * (t + 0.3).cos(), 12. + 7. * (t + 0.3).sin()],
                    ],
                    Dark,
                );
            }
        }
        Zoom => {
            d.oval(9., 9., 6., 6., Some(Pale), Dark);
            d.path(&[[14., 14.], [21., 21.]], Dark);
            d.star(9., 9., 3., Blue);
        }
        MagicWand => {
            d.polygon(&[[3., 19.], [14., 8.], [17., 11.], [6., 22.]], Dark);
            d.path(&[[12., 10.], [15., 13.]], White);
            d.star(17., 5., 3., Gold);
            d.star(6., 7., 2., Gold);
            d.star(21., 12., 2., Gold);
        }
        Pan => d.polygon(
            &[
                [3., 11.],
                [5., 10.],
                [8., 13.],
                [7., 5.],
                [9., 4.],
                [10., 11.],
                [10., 2.],
                [12., 2.],
                [13., 11.],
                [14., 4.],
                [16., 4.],
                [16., 12.],
                [18., 7.],
                [20., 8.],
                [19., 18.],
                [16., 22.],
                [10., 22.],
                [6., 18.],
            ],
            White,
        ),
        Bucket => {
            d.polygon(
                &[[4., 7.], [11., 3.], [19., 11.], [10., 20.], [2., 12.]],
                Pale,
            );
            d.path(&[[6., 8.], [6., 4.], [10., 2.], [14., 5.]], Dark);
            d.path(&[[4., 12.], [17., 12.]], Blue);
            d.polygon(
                &[[20., 14.], [18., 19.], [19., 22.], [22., 22.], [23., 19.]],
                Blue,
            );
        }
        Gradient => {
            for n in 0..6 {
                let ink = [Dark, Blue, Pale, Pale, White, White][n];
                d.rect(3. + n as f32 * 3., 4., 3., 16., Some(ink), ink);
            }
            d.rect(3., 4., 18., 16., None, Dark);
            d.path(&[[4., 20.], [20., 4.]], Gold);
            d.oval(4., 20., 1.5, 1.5, Some(White), Dark);
            d.oval(20., 4., 1.5, 1.5, Some(White), Dark);
        }
        BrushCircle => {
            d.oval(12., 12., 8., 8., Some(Blue), Dark);
            d.oval(9., 9., 2., 2., Some(White), White);
        }
        BrushSquare => {
            d.rect(4., 4., 16., 16., Some(Blue), Dark);
            d.rect(7., 7., 4., 4., Some(White), White);
        }
        BrushHardness => {
            d.oval(12., 12., 10., 10., Some(Pale), Pale);
            d.oval(12., 12., 7., 7., Some(Blue), Blue);
            d.oval(12., 12., 4., 4., Some(Dark), Dark);
        }
        BrushSpacing => {
            for x in [4., 12., 20.] {
                d.oval(x, 9., 2.5, 2.5, Some(Blue), Dark);
            }
            d.path(&[[4., 17.], [20., 17.]], Dark);
            d.path(&[[4., 15.], [4., 20.]], Dark);
            d.path(&[[20., 15.], [20., 20.]], Dark);
        }
        BrushAntialias => {
            for n in 0..4 {
                d.rect(
                    3. + n as f32 * 4.,
                    17. - n as f32 * 4.,
                    4.,
                    5. + n as f32 * 4.,
                    Some(Blue),
                    Blue,
                );
            }
            d.path(&[[2., 16.], [17., 1.], [22., 1.]], Pale);
        }
        BrushSmoothing => {
            d.path(&[[2., 18.], [6., 5.], [13., 20.], [21., 5.]], Pale);
            let points: Vec<_> = (0..24)
                .map(|i| {
                    let t = i as f32 / 23.;
                    [2. + 20. * t, 12. + 6. * (t * std::f32::consts::TAU).cos()]
                })
                .collect();
            d.path(&points, Blue);
            d.oval(2., 18., 1.5, 1.5, Some(Gold), Dark);
            d.oval(22., 18., 1.5, 1.5, Some(Gold), Dark);
        }
        BrushPressure => {
            d.polygon(
                &[[4., 22.], [20., 22.], [15., 17.], [12., 16.], [9., 18.]],
                Blue,
            );
            d.polygon(&[[9., 11.], [15., 11.], [12., 16.]], Dark);
            d.path(&[[12., 2.], [12., 10.]], Dark);
            d.path(&[[6., 6.], [8., 10.]], Gold);
            d.path(&[[18., 6.], [16., 10.]], Gold);
        }
        Brush => {
            d.polygon(&[[8., 13.], [17., 2.], [20., 3.], [12., 16.]], Gold);
            d.polygon(
                &[
                    [8., 12.],
                    [13., 16.],
                    [10., 21.],
                    [3., 22.],
                    [6., 18.],
                    [6., 15.],
                ],
                Blue,
            );
            d.path(&[[8., 13.], [12., 16.]], White);
        }
        Eraser => {
            d.polygon(
                &[[3., 15.], [13., 4.], [22., 12.], [12., 22.], [9., 22.]],
                Purple,
            );
            d.polygon(
                &[[3., 15.], [8., 10.], [17., 18.], [12., 22.], [9., 22.]],
                White,
            );
            d.path(&[[3., 23.], [21., 23.]], Dark);
        }
        Pencil => {
            d.polygon(
                &[[3., 21.], [5., 14.], [16., 3.], [21., 8.], [10., 19.]],
                Gold,
            );
            d.polygon(&[[3., 21.], [5., 14.], [10., 19.]], White);
            d.polygon(&[[3., 21.], [4., 18.], [6., 20.]], Dark);
            d.polygon(&[[16., 3.], [18., 1.], [23., 6.], [21., 8.]], Red);
            d.path(&[[7., 15.], [18., 4.]], White);
        }
        Picker => {
            d.polygon(
                &[
                    [4., 18.],
                    [6., 13.],
                    [14., 5.],
                    [19., 10.],
                    [11., 18.],
                    [6., 20.],
                ],
                Pale,
            );
            d.polygon(&[[13., 5.], [16., 2.], [22., 8.], [19., 11.]], Dark);
            d.path(&[[5., 18.], [11., 12.]], Blue);
            d.oval(3., 22., 1., 1., Some(Blue), Blue);
        }
        CloneStamp => {
            d.polygon(
                &[
                    [8., 15.],
                    [8., 11.],
                    [10., 9.],
                    [9., 4.],
                    [11., 2.],
                    [14., 2.],
                    [16., 4.],
                    [15., 9.],
                    [17., 11.],
                    [17., 15.],
                ],
                Gold,
            );
            d.rect(4., 15., 17., 5., Some(Pale), Dark);
            d.path(&[[3., 22.], [22., 22.]], Blue);
        }
        Recolor => {
            d.oval(7., 7., 5., 5., Some(Red), Red);
            d.oval(17., 17., 5., 5., Some(Blue), Blue);
            d.arrow([14., 3.], [21., 10.], Dark);
            d.arrow([10., 21.], [3., 14.], Dark);
        }
        Text => {
            d.polygon(
                &[
                    [3., 3.],
                    [21., 3.],
                    [21., 7.],
                    [19., 7.],
                    [19., 5.],
                    [14., 5.],
                    [14., 20.],
                    [17., 20.],
                    [17., 22.],
                    [7., 22.],
                    [7., 20.],
                    [10., 20.],
                    [10., 5.],
                    [5., 5.],
                    [5., 7.],
                    [3., 7.],
                ],
                Blue,
            );
        }
        Line => {
            d.path(&[[3., 20.], [8., 15.], [17., 13.], [21., 4.]], Blue);
            for (x, y) in [(3., 20.), (8., 15.), (17., 13.), (21., 4.)] {
                d.rect(x - 1.5, y - 1.5, 3., 3., Some(White), Dark);
            }
        }
        Shapes => {
            d.rect(2., 3., 12., 11., Some(Pale), Blue);
            d.oval(16., 16., 6., 6., Some(Gold), Dark);
            d.polygon(&[[2., 22.], [7., 12.], [12., 22.]], Green);
        }
        DragDrop => {
            d.dash_rect(11., 11., 11., 11.);
            d.page(2., 1.);
            d.arrow([9., 10.], [18., 19.], Blue);
        }
        New => {
            d.page(4., 2.);
            d.star(18., 18., 5., Green);
        }
        Open => {
            d.polygon(
                &[
                    [2., 6.],
                    [8., 6.],
                    [10., 9.],
                    [21., 9.],
                    [21., 21.],
                    [2., 21.],
                ],
                Gold,
            );
            d.polygon(&[[6., 12.], [23., 12.], [19., 21.], [2., 21.]], Pale);
        }
        Save => {
            d.polygon(
                &[[3., 2.], [19., 2.], [22., 5.], [22., 22.], [3., 22.]],
                Blue,
            );
            d.rect(7., 2., 10., 7., Some(White), Dark);
            d.rect(7., 14., 11., 8., Some(White), Dark);
            d.path(&[[14., 3.], [14., 7.]], Blue);
        }
        Print => {
            d.rect(6., 2., 12., 7., Some(White), Dark);
            d.rect(2., 8., 20., 11., Some(Pale), Dark);
            d.rect(6., 14., 12., 9., Some(White), Dark);
            d.oval(19., 11., 1., 1., Some(Green), Green);
            d.path(&[[8., 17.], [16., 17.]], Dark);
            d.path(&[[8., 20.], [16., 20.]], Dark);
        }
        Cut => {
            d.oval(5., 18., 3., 3., None, Blue);
            d.oval(12., 18., 3., 3., None, Blue);
            d.path(&[[7., 16.], [18., 2.], [12., 15.], [7., 2.]], Dark);
            d.oval(10., 12., 1., 1., Some(Gold), Gold);
        }
        Copy | Duplicate => {
            d.page(2., 1.);
            d.page(8., 6.);
            if icon == Duplicate {
                d.star(15., 16., 3., Green);
            }
        }
        Paste => {
            d.rect(4., 4., 16., 18., Some(Gold), Dark);
            d.rect(8., 2., 8., 4., Some(Pale), Dark);
            d.page(8., 7.);
        }
        Crop => {
            d.path(&[[2., 7.], [17., 7.], [17., 22.]], Dark);
            d.path(&[[7., 2.], [7., 17.], [22., 17.]], Dark);
            d.path(&[[3., 21.], [21., 3.]], Blue);
        }
        Deselect => {
            d.dash_rect(2., 3., 17., 16.);
            d.path(&[[13., 14.], [22., 23.]], Red);
            d.path(&[[22., 14.], [13., 23.]], Red);
        }
        Undo | Redo => {
            d.path(
                &[[5., 8.], [13., 6.], [19., 9.], [21., 14.], [19., 20.]],
                Blue,
            );
            d.polygon(&[[2., 9.], [8., 3.], [8., 7.], [12., 11.]], Blue);
            if icon == Redo {
                for mark in &mut d.0 {
                    for p in &mut mark.points {
                        p[0] = 24. - p[0];
                    }
                }
            }
        }
        Grid => {
            d.rect(2., 2., 20., 20., None, Dark);
            for n in [7., 12., 17.] {
                d.path(&[[n, 2.], [n, 22.]], Blue);
                d.path(&[[2., n], [22., n]], Blue);
            }
        }
        Rulers => {
            d.polygon(
                &[
                    [2., 2.],
                    [7., 2.],
                    [7., 17.],
                    [22., 17.],
                    [22., 22.],
                    [2., 22.],
                ],
                Gold,
            );
            for n in [5., 9., 13.] {
                d.path(&[[3., n], [6., n]], Dark);
            }
            for n in [10., 14., 18.] {
                d.path(&[[n, 18.], [n, 21.]], Dark);
            }
        }
        Tools | Properties => {
            d.polygon(
                &[
                    [3., 21.],
                    [1., 19.],
                    [14., 6.],
                    [13., 2.],
                    [17., 1.],
                    [16., 5.],
                    [19., 8.],
                    [23., 7.],
                    [22., 12.],
                    [18., 12.],
                    [6., 24.],
                ],
                Pale,
            );
            if icon == Tools {
                d.path(&[[3., 2.], [22., 21.]], Gold);
                d.rect(1., 1., 5., 5., Some(Gold), Dark);
            }
        }
        History => {
            d.oval(12., 12., 9., 9., Some(White), Blue);
            d.path(&[[12., 5.], [12., 12.], [17., 15.]], Dark);
            d.arrow([3., 3.], [3., 9.], Blue);
        }
        Layers => {
            for y in [15., 9., 3.] {
                d.polygon(
                    &[[2., y + 5.], [12., y], [22., y + 5.], [12., y + 10.]],
                    if y == 3. { Pale } else { White },
                );
            }
        }
        Colors | HueSaturation => {
            for (x, y, ink) in [(8., 8., Red), (16., 8., Green), (12., 16., Blue)] {
                d.oval(x, y, 6., 6., Some(ink), ink);
            }
            d.oval(12., 11., 2., 2., Some(White), White);
        }
        Add => {
            d.rect(3., 3., 16., 18., Some(White), Blue);
            d.star(17., 17., 6., Green);
        }
        Delete => {
            d.rect(6., 7., 12., 15., Some(Pale), Dark);
            d.path(&[[4., 5.], [20., 5.]], Dark);
            d.path(&[[9., 5.], [9., 2.], [15., 2.], [15., 5.]], Dark);
            for x in [9., 12., 15.] {
                d.path(&[[x, 10.], [x, 19.]], Blue);
            }
        }
        Up => d.arrow([12., 22.], [12., 2.], Blue),
        Down => d.arrow([12., 2.], [12., 22.], Blue),
        Merge => {
            d.rect(3., 2., 10., 7., Some(Pale), Blue);
            d.rect(10., 15., 11., 7., Some(White), Blue);
            d.arrow([7., 10.], [14., 15.], Green);
        }
        Close => {
            d.path(&[[5., 5.], [19., 19.]], Dark);
            d.path(&[[19., 5.], [5., 19.]], Dark);
        }
        FlipHorizontal | FlipVertical => {
            d.polygon(&[[3., 5.], [10., 11.], [3., 17.]], Blue);
            d.polygon(&[[21., 5.], [14., 11.], [21., 17.]], Pale);
            d.path(&[[12., 2.], [12., 22.]], Dark);
            if icon == FlipVertical {
                for m in &mut d.0 {
                    for p in &mut m.points {
                        p.swap(0, 1);
                    }
                }
            }
        }
        Rotate => {
            d.rect(7., 7., 10., 10., Some(Pale), Blue);
            d.path(&[[2., 15.], [2., 7.], [6., 2.], [16., 2.], [21., 6.]], Blue);
            d.arrow([16., 2.], [21., 6.], Blue);
        }
        Resize => {
            d.rect(2., 11., 10., 11., Some(Pale), Blue);
            d.dash_rect(2., 2., 20., 20.);
            d.arrow([9., 15.], [21., 3.], Dark);
        }
        CanvasSize => {
            d.dash_rect(2., 2., 20., 20.);
            d.rect(7., 7., 10., 10., Some(Pale), Blue);
            d.arrow([12., 7.], [12., 2.], Dark);
            d.arrow([17., 12.], [22., 12.], Dark);
        }
        Flatten => {
            d.polygon(&[[2., 6.], [12., 2.], [22., 6.], [12., 10.]], Pale);
            d.arrow([12., 11.], [12., 17.], Blue);
            d.rect(3., 18., 18., 4., Some(White), Dark);
        }
        AutoLevel => {
            d.path(&[[3., 21.], [3., 4.]], Dark);
            d.path(&[[3., 21.], [21., 21.]], Dark);
            d.path(
                &[
                    [5., 18.],
                    [8., 18.],
                    [8., 9.],
                    [11., 9.],
                    [11., 14.],
                    [14., 14.],
                    [14., 5.],
                    [17., 5.],
                    [17., 12.],
                    [20., 12.],
                ],
                Blue,
            );
            d.star(19., 3., 2., Gold);
        }
        BlackWhite | InvertColors | InvertAlpha => {
            d.oval(12., 12., 10., 10., Some(White), Dark);
            d.polygon(
                &[
                    [12., 2.],
                    [6., 4.],
                    [2., 10.],
                    [3., 16.],
                    [8., 21.],
                    [12., 22.],
                ],
                if icon == InvertColors { Blue } else { Dark },
            );
            if icon == InvertAlpha {
                d.rect(13., 5., 4., 4., Some(Pale), Pale);
                d.rect(17., 9., 4., 4., Some(Pale), Pale);
                d.rect(13., 13., 4., 4., Some(Pale), Pale);
                d.rect(17., 17., 3., 3., Some(Pale), Pale);
            }
            if icon == InvertColors {
                d.arrow([8., 6.], [17., 6.], Gold);
                d.arrow([16., 18.], [7., 18.], Blue);
            }
        }
        BrightnessContrast | Exposure => {
            d.oval(12., 12., 5., 5., Some(Gold), Dark);
            for (a, b) in [
                ([12., 1.], [12., 4.]),
                ([12., 20.], [12., 23.]),
                ([1., 12.], [4., 12.]),
                ([20., 12.], [23., 12.]),
                ([4., 4.], [6., 6.]),
                ([18., 18.], [20., 20.]),
                ([4., 20.], [6., 18.]),
                ([18., 6.], [20., 4.]),
            ] {
                d.path(&[a, b], Gold);
            }
            if icon == BrightnessContrast {
                d.polygon(
                    &[[12., 7.], [8., 9.], [7., 12.], [9., 16.], [12., 17.]],
                    Dark,
                );
            } else {
                d.star(19., 19., 3., Blue);
            }
        }
        Curves => {
            d.path(&[[3., 2.], [3., 21.], [22., 21.]], Dark);
            d.path(
                &[
                    [4., 20.],
                    [7., 19.],
                    [10., 16.],
                    [12., 10.],
                    [16., 5.],
                    [21., 3.],
                ],
                Blue,
            );
            for (x, y) in [(7., 19.), (12., 10.), (21., 3.)] {
                d.rect(x - 1., y - 1., 2., 2., Some(White), Dark);
            }
        }
        HighlightsShadows => {
            d.rect(3., 3., 8., 18., Some(Dark), Dark);
            d.rect(13., 3., 8., 18., Some(Gold), Gold);
            d.arrow([5., 18.], [9., 6.], White);
            d.arrow([15., 6.], [19., 18.], Dark);
        }
        Levels => {
            for (x, h) in [
                (3., 5.),
                (6., 9.),
                (9., 17.),
                (12., 14.),
                (15., 8.),
                (18., 5.),
            ] {
                d.rect(x, 20. - h, 2., h, Some(Blue), Blue);
            }
            d.polygon(&[[3., 23.], [6., 19.], [9., 23.]], Dark);
            d.polygon(&[[16., 23.], [19., 19.], [22., 23.]], White);
        }
        Posterize => {
            for (x, y, ink) in [
                (2., 15., Blue),
                (7., 10., Green),
                (12., 5., Gold),
                (17., 2., Red),
            ] {
                d.rect(x, y, 5., 22. - y, Some(ink), ink);
            }
        }
        Sepia => {
            d.picture();
            for mark in &mut d.0 {
                if mark.stroke.is_some() {
                    mark.stroke = Some(Gold);
                }
                if mark.fill == Some(Pale) {
                    mark.fill = Some(Gold);
                }
            }
        }
        TemperatureTint => {
            d.oval(8., 18., 4., 4., Some(Red), Dark);
            d.rect(6., 3., 4., 14., Some(White), Dark);
            d.path(&[[8., 8.], [8., 19.]], Red);
            d.star(18., 7., 4., Blue);
            d.path(&[[15., 4.], [21., 10.]], Blue);
            d.path(&[[15., 10.], [21., 4.]], Blue);
        }
        InkSketch => {
            d.picture();
            d.path(&[[7., 20.], [18., 3.], [21., 6.], [11., 21.]], Dark);
        }
        OilPainting => {
            d.path(&[[3., 16.], [5., 7.], [9., 6.], [8., 19.]], Gold);
            d.path(&[[11., 20.], [13., 5.], [16., 4.], [16., 19.]], Blue);
            d.path(&[[19., 18.], [21., 9.]], Red);
        }
        PencilSketch => {
            d.path(
                &[
                    [3., 16.],
                    [5., 10.],
                    [8., 13.],
                    [11., 7.],
                    [14., 11.],
                    [18., 4.],
                ],
                Dark,
            );
            d.path(&[[8., 22.], [19., 11.], [22., 14.], [11., 23.]], Gold);
        }
        Bokeh => {
            d.oval(7., 8., 5., 5., Some(Pale), Blue);
            d.oval(17., 7., 3., 3., Some(Gold), Gold);
            d.oval(14., 17., 6., 6., None, Purple);
        }
        Fragment => {
            for (x, y) in [(2., 3.), (13., 2.), (7., 10.), (15., 15.)] {
                d.rect(x, y, 6., 6., Some(Pale), Blue);
            }
        }
        GaussianBlur => {
            for (r, ink) in [(10., White), (8., Pale), (5., Blue), (2., Dark)] {
                d.oval(12., 12., r, r, Some(ink), ink);
            }
        }
        Median => {
            for (x, h) in [(3., 5.), (7., 11.), (11., 18.), (15., 13.), (19., 6.)] {
                d.rect(x, 21. - h, 2., h, Some(Pale), Blue);
            }
            d.arrow([12., 1.], [12., 9.], Red);
        }
        MotionBlur => {
            for (y, x) in [(5., 3.), (10., 2.), (15., 5.)] {
                d.path(&[[x, y], [x + 12., y]], Blue);
            }
            d.rect(16., 6., 6., 12., Some(Pale), Blue);
        }
        RadialBlur => {
            for n in 0..3 {
                let r = 4. + n as f32 * 3.;
                let points: Vec<_> = (0..20)
                    .map(|a| {
                        let t = a as f32 * 0.18;
                        [12. + r * t.cos(), 12. + r * t.sin()]
                    })
                    .collect();
                d.path(&points, Blue);
            }
            d.oval(12., 12., 1., 1., Some(Dark), Dark);
        }
        SketchBlur => {
            for y in [6., 11., 16.] {
                d.wave(y, y, Dark);
            }
        }
        SquareBlur => {
            for (x, s, ink) in [(2., 20., White), (5., 14., Pale), (8., 8., Blue)] {
                d.rect(x, x, s, s, Some(ink), ink);
            }
        }
        SurfaceBlur => {
            d.rect(3., 3., 9., 18., Some(Pale), Blue);
            for y in [5., 10., 15.] {
                d.wave(y, 0., Blue);
            }
            d.path(&[[12., 2.], [12., 22.]], Dark);
        }
        ZoomBlur => {
            for (a, b) in [
                ([2., 2.], [9., 9.]),
                ([22., 2.], [15., 9.]),
                ([2., 22.], [9., 15.]),
                ([22., 22.], [15., 15.]),
                ([12., 1.], [12., 7.]),
                ([12., 23.], [12., 17.]),
            ] {
                d.path(&[a, b], Blue);
            }
            d.oval(12., 12., 3., 3., Some(Pale), Blue);
        }
        Quantize => {
            for (n, ink) in [Red, Gold, Green, Blue, Purple, Dark]
                .into_iter()
                .enumerate()
            {
                d.rect(
                    3. + (n % 3) as f32 * 6.,
                    4. + (n / 3) as f32 * 8.,
                    5.,
                    7.,
                    Some(ink),
                    ink,
                );
            }
        }
        Bulge => {
            d.oval(12., 12., 10., 10., None, Blue);
            d.oval(12., 12., 5., 10., None, Blue);
            d.oval(12., 12., 10., 5., None, Blue);
            d.path(&[[2., 12.], [22., 12.]], Blue);
        }
        Crystalize => {
            for (points, ink) in [
                (&[[2., 3.], [12., 2.], [8., 13.], [2., 10.]][..], Pale),
                (&[[12., 2.], [22., 7.], [16., 15.], [8., 13.]], Blue),
                (&[[2., 10.], [8., 13.], [10., 22.], [2., 20.]], Gold),
                (&[[8., 13.], [16., 15.], [22., 22.], [10., 22.]], White),
            ] {
                d.polygon(points, ink);
            }
        }
        Dents => {
            for y in [5., 12., 19.] {
                d.wave(y, 0., Blue);
            }
            d.path(&[[4., 2.], [8., 8.], [5., 15.], [9., 22.]], Dark);
        }
        FrostedGlass => {
            d.rect(3., 3., 18., 18., Some(Pale), Blue);
            for (x, y) in [
                (6., 7.),
                (14., 5.),
                (18., 10.),
                (10., 12.),
                (6., 17.),
                (17., 18.),
            ] {
                d.path(&[[x - 1., y + 1.], [x + 2., y - 2.]], White);
            }
        }
        Morphology => {
            d.rect(4., 4., 16., 16., Some(Pale), Blue);
            d.rect(8., 8., 8., 8., Some(Blue), Blue);
            d.arrow([7., 7.], [3., 3.], Dark);
            d.arrow([17., 17.], [21., 21.], Dark);
        }
        Pixelate => {
            for y in 0..3 {
                for x in 0..3 {
                    let ink = if (x + y) % 2 == 0 { Blue } else { Pale };
                    d.rect(
                        3. + x as f32 * 6.,
                        3. + y as f32 * 6.,
                        6.,
                        6.,
                        Some(ink),
                        White,
                    );
                }
            }
        }
        PolarInversion => {
            d.oval(12., 12., 9., 9., None, Blue);
            d.oval(12., 12., 4., 4., Some(Pale), Blue);
            d.arrow([2., 12.], [7., 12.], Dark);
            d.arrow([22., 12.], [17., 12.], Dark);
        }
        TileReflection => {
            for x in [3., 12.] {
                for y in [3., 12.] {
                    d.rect(x, y, 9., 9., Some(Pale), Blue);
                    d.path(&[[x, y + 9.], [x + 9., y]], White);
                }
            }
        }
        Twist => {
            let points: Vec<_> = (0..60)
                .map(|n| {
                    let t = n as f32 * 0.18;
                    let r = n as f32 / 6.;
                    [12. + r * t.cos(), 12. + r * t.sin()]
                })
                .collect();
            d.path(&points, Blue);
        }
        AddNoise | ReduceNoise => {
            for (x, y) in [
                (3., 4.),
                (9., 7.),
                (15., 3.),
                (21., 8.),
                (5., 15.),
                (12., 13.),
                (18., 17.),
                (4., 21.),
                (13., 21.),
            ] {
                d.rect(x, y, 2., 2., Some(Dark), Dark);
            }
            if icon == AddNoise {
                d.star(18., 18., 5., Green);
            } else {
                d.path(&[[13., 18.], [23., 18.]], Red);
            }
        }
        DropShadow => {
            d.rect(8., 8., 14., 14., Some(Dark), Dark);
            d.rect(3., 3., 14., 14., Some(White), Blue);
        }
        Glow => {
            for r in [10., 7., 4.] {
                d.oval(12., 12., r, r, None, Gold);
            }
            d.star(12., 12., 4., White);
        }
        RedEye => {
            d.oval(12., 12., 10., 6., Some(White), Dark);
            d.oval(12., 12., 4., 4., Some(Red), Red);
            d.path(&[[3., 22.], [22., 3.]], Blue);
        }
        Sharpen => {
            d.polygon(&[[3., 21.], [12., 2.], [21., 21.]], Pale);
            d.path(&[[12., 2.], [12., 21.]], Dark);
            d.path(&[[8., 14.], [16., 14.]], Blue);
        }
        SoftenPortrait => {
            d.oval(12., 8., 4., 5., Some(Pale), Blue);
            d.path(
                &[
                    [3., 22.],
                    [5., 17.],
                    [9., 15.],
                    [15., 15.],
                    [19., 17.],
                    [21., 22.],
                ],
                Blue,
            );
            d.star(20., 5., 2., Gold);
        }
        Straighten => {
            d.polygon(&[[3., 8.], [18., 3.], [22., 17.], [7., 22.]], Pale);
            d.path(&[[1., 13.], [23., 13.]], Dark);
            d.arrow([12., 2.], [12., 8.], Blue);
        }
        Vignette => {
            d.rect(2., 3., 20., 18., Some(Dark), Dark);
            d.oval(12., 12., 8., 7., Some(Pale), Pale);
            d.oval(12., 12., 5., 4., Some(White), White);
        }
        Clouds => {
            d.oval(8., 13., 5., 5., Some(Pale), Blue);
            d.oval(14., 9., 6., 6., Some(White), Blue);
            d.oval(19., 14., 4., 4., Some(White), Blue);
            d.rect(6., 14., 14., 5., Some(White), White);
            d.path(&[[5., 19.], [20., 19.]], Blue);
        }
        Julia => {
            d.oval(8., 8., 5., 4., None, Purple);
            d.oval(16., 16., 5., 4., None, Blue);
            d.path(
                &[
                    [3., 21.],
                    [8., 15.],
                    [10., 11.],
                    [14., 13.],
                    [17., 9.],
                    [21., 3.],
                ],
                Purple,
            );
        }
        Mandelbrot => {
            d.oval(14., 12., 7., 8., Some(Blue), Blue);
            d.oval(6., 12., 4., 4., Some(Blue), Blue);
            d.oval(3., 12., 2., 2., Some(Purple), Purple);
            d.oval(15., 3., 2., 2., Some(Pale), Blue);
        }
        Turbulence => {
            d.wave(5., 1., Blue);
            d.wave(11., 3., Purple);
            d.wave(17., 5., Blue);
        }
        EdgeDetect => {
            d.picture();
            d.path(&[[7., 18.], [10., 12.], [14., 16.], [17., 11.]], Red);
        }
        Emboss => {
            d.polygon(
                &[[3., 17.], [12., 3.], [21., 17.], [16., 22.], [8., 22.]],
                Pale,
            );
            d.path(&[[3., 17.], [12., 3.], [21., 17.]], White);
            d.path(&[[4., 19.], [12., 7.], [20., 19.]], Dark);
        }
        Outline => {
            d.oval(12., 12., 9., 8., None, Blue);
            d.oval(12., 12., 5., 4., None, Dark);
        }
        Relief => {
            d.polygon(
                &[[2., 21.], [8., 9.], [12., 15.], [17., 4.], [22., 21.]],
                Gold,
            );
            d.path(&[[2., 21.], [8., 9.], [12., 15.], [17., 4.]], White);
            d.path(&[[9., 11.], [13., 17.], [18., 6.]], Dark);
        }
    }
    d
}
// Ear clipping preserves concave silhouettes (hand, brush, cursor, folder).
// egui's filled PathShape only supports convex polygons.
fn triangulate(points: &[Pos2]) -> Vec<u32> {
    let cross = |a: Pos2, b: Pos2, c: Pos2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let area: f32 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum();
    let sign = area.signum();
    let mut remaining: Vec<_> = (0..points.len()).collect();
    let mut indices = Vec::new();
    while remaining.len() > 2 {
        let ear = (0..remaining.len()).find(|&i| {
            let [a, b, c] = [
                remaining[(i + remaining.len() - 1) % remaining.len()],
                remaining[i],
                remaining[(i + 1) % remaining.len()],
            ];
            if cross(points[a], points[b], points[c]) * sign <= 0.0001 {
                return false;
            }
            !remaining.iter().any(|&j| {
                j != a
                    && j != b
                    && j != c
                    && cross(points[a], points[b], points[j]) * sign >= -0.0001
                    && cross(points[b], points[c], points[j]) * sign >= -0.0001
                    && cross(points[c], points[a], points[j]) * sign >= -0.0001
            })
        });
        let Some(i) = ear else {
            break;
        };
        indices.extend([
            remaining[(i + remaining.len() - 1) % remaining.len()] as u32,
            remaining[i] as u32,
            remaining[(i + 1) % remaining.len()] as u32,
        ]);
        remaining.remove(i);
    }
    indices
}
/// Draw vector paths at the requested DPI, with consistent disabled colors.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, enabled: bool) {
    let scale = rect.size() / 24.;
    let map = |p: [f32; 2]| Pos2::new(rect.left() + p[0] * scale.x, rect.top() + p[1] * scale.y);
    let ink = |c: Ink| {
        if enabled {
            c.rgb()
        } else {
            let c = c.rgb();
            Color32::from_gray(
                ((u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b())) / 6 + 115) as u8,
            )
        }
    };
    for m in artwork(icon).0 {
        let points: Vec<_> = m.points.into_iter().map(map).collect();
        let stroke = m.stroke.map_or(Stroke::NONE, |c| {
            Stroke::new((rect.width() / 24. * 1.3).max(0.7), ink(c))
        });
        if m.closed {
            if let Some(fill) = m.fill {
                let mesh = egui::Mesh {
                    vertices: points
                        .iter()
                        .map(|&pos| egui::epaint::Vertex {
                            pos,
                            uv: egui::epaint::WHITE_UV,
                            color: ink(fill),
                        })
                        .collect(),
                    indices: triangulate(&points),
                    ..Default::default()
                };
                painter.add(egui::Shape::mesh(mesh));
            }
            painter.add(egui::Shape::closed_line(points, stroke));
        } else {
            painter.add(egui::Shape::line(points, stroke));
        }
    }
}
/// Export the exact UI artwork for visual review; all geometry is authored above.
pub fn atlas_svg() -> String {
    let columns = 8;
    let rows = Icon::ALL.len().div_ceil(columns);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1120\" height=\"{}\" viewBox=\"0 0 1120 {}\"><rect width=\"100%\" height=\"100%\" fill=\"#f8f8f8\"/>",
        rows * 90,
        rows * 90
    );
    for (i, &icon) in Icon::ALL.iter().enumerate() {
        let x = (i % columns) * 140;
        let y = (i / columns) * 90;
        let _ = write!(
            svg,
            "<g transform=\"translate({},{}) scale(1.8)\" stroke-linecap=\"round\" stroke-linejoin=\"round\">",
            x + 48,
            y + 6
        );
        for mark in artwork(icon).0 {
            let color = |ink: Option<Ink>| {
                ink.map_or_else(
                    || "none".to_owned(),
                    |c| {
                        let c = c.rgb();
                        format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
                    },
                )
            };
            let points = mark
                .points
                .iter()
                .map(|p| format!("{},{}", p[0], p[1]))
                .collect::<Vec<_>>()
                .join(" ");
            let _ = write!(
                svg,
                "<{} points=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.3\"/>",
                if mark.closed { "polygon" } else { "polyline" },
                points,
                color(mark.fill),
                color(mark.stroke)
            );
        }
        let _ = write!(
            svg,
            "</g><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-family=\"sans-serif\" font-size=\"11\" fill=\"#394149\">{}</text>",
            x + 70,
            y + 68,
            icon.name()
        );
    }
    svg.push_str("</svg>");
    svg
}
