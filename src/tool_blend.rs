//! Shared drawing blend choices from Paint.NET's ContentBlendMode.
use crate::document::{BlendMode, Pixel, blend, scale};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolBlend {
    #[default]
    Normal,
    Composite(BlendMode),
    Overwrite,
}
impl ToolBlend {
    pub fn choices() -> impl Iterator<Item = Self> {
        BlendMode::ALL
            .into_iter()
            .map(Self::composite)
            .chain([Self::Overwrite])
    }
    pub fn composite(mode: BlendMode) -> Self {
        if mode == BlendMode::Normal {
            Self::Normal
        } else {
            Self::Composite(mode)
        }
    }
    pub fn mode(self) -> BlendMode {
        match self {
            Self::Composite(mode) => mode,
            _ => BlendMode::Normal,
        }
    }
    pub fn name(self) -> &'static str {
        if self == Self::Overwrite {
            "Overwrite"
        } else {
            self.mode().name()
        }
    }
    /// Coverage is independent of the content's alpha. Overwrite uses the
    /// recovered ColorBgra32 alpha-weighted interpolation, not source-over.
    pub fn apply(self, dst: Pixel, src: Pixel, coverage: u8) -> Pixel {
        if self != Self::Overwrite {
            return blend(dst, src, coverage, self.mode());
        }
        match coverage {
            0 => dst,
            255 => src,
            _ => {
                let da = scale(255 - u32::from(coverage), u32::from(dst[3]));
                let sa = scale(u32::from(coverage), u32::from(src[3]));
                let alpha = da + sa;
                let mut out = [0, 0, 0, alpha as u8];
                for c in 0..3 {
                    out[c] = (u32::from(dst[c]) * da + u32::from(src[c]) * sa)
                        .checked_div(alpha)
                        .unwrap_or(0) as u8;
                }
                out
            }
        }
    }
}
