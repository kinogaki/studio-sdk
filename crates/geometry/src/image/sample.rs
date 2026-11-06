//! [`Sample`]: what one channel's value is stored as, and half floats.

/// What a channel's value is stored as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Sample {
    /// 8 bits, 0 to 255 for 0 to 1.
    U8,
    /// 16 bits, 0 to 65535 for 0 to 1.
    U16,
    /// A half float.
    F16,
    /// A float.
    F32,
    /// 32 bits, a whole number (an id; OpenEXR's UINT).
    U32,
}

impl Sample {
    /// Bytes a value takes.
    pub const fn bytes(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 | Self::F16 => 2,
            Self::F32 | Self::U32 => 4,
        }
    }

    /// Its name in files.
    pub const fn name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::F16 => "f16",
            Self::F32 => "f32",
            Self::U32 => "u32",
        }
    }

    /// The sample named `name` ([`name`](Self::name)).
    pub fn named(name: &str) -> Option<Self> {
        [Self::U8, Self::U16, Self::F16, Self::F32, Self::U32]
            .into_iter()
            .find(|s| s.name() == name)
    }

    /// The first value of `bytes` (native order), as a float: a U32 its
    /// number, exact up to 2²⁴.
    pub fn read(self, bytes: &[u8]) -> f32 {
        let four = || [bytes[0], bytes[1], bytes[2], bytes[3]];
        match self {
            Self::U8 => f32::from(bytes[0]) / 255.0,
            Self::U16 => f32::from(u16::from_ne_bytes([bytes[0], bytes[1]])) / 65535.0,
            Self::F16 => half_to_f32(u16::from_ne_bytes([bytes[0], bytes[1]])),
            Self::F32 => f32::from_ne_bytes(four()),
            Self::U32 => u32::from_ne_bytes(four()) as f32,
        }
    }

    /// `value` written over the first value of `bytes` (native order): a
    /// U32 its nearest whole number, saturated.
    pub fn write(self, value: f32, bytes: &mut [u8]) {
        let unit = |top: f32| (value.clamp(0.0, 1.0) * top).round();
        match self {
            Self::U8 => bytes[0] = unit(255.0) as u8,
            Self::U16 => bytes[..2].copy_from_slice(&(unit(65535.0) as u16).to_ne_bytes()),
            Self::F16 => bytes[..2].copy_from_slice(&f32_to_half(value).to_ne_bytes()),
            Self::F32 => bytes[..4].copy_from_slice(&value.to_ne_bytes()),
            Self::U32 => bytes[..4].copy_from_slice(&(value.round() as u32).to_ne_bytes()),
        }
    }
}

/// A half float's value.
pub fn half_to_f32(half: u16) -> f32 {
    let sign = u32::from(half & 0x8000) << 16;
    let exponent = u32::from(half >> 10 & 0x1f);
    let fraction = u32::from(half & 0x3ff);
    f32::from_bits(match (exponent, fraction) {
        (0, 0) => sign,
        (0, f) => {
            // Subnormal: shifted up until its leading bit leaves the fraction.
            let shift = f.leading_zeros() - 21;
            sign | (127 - 15 + 1 - shift) << 23 | ((f << shift) & 0x3ff) << 13
        }
        (31, f) => sign | 0xff << 23 | f << 13,
        (e, f) => sign | (e + 127 - 15) << 23 | f << 13,
    })
}

/// `value` as the nearest half float.
pub fn f32_to_half(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = (bits >> 16 & 0x8000) as u16;
    let exponent = (bits >> 23 & 0xff) as i32;
    let fraction = bits & 0x7f_ffff;
    if exponent == 255 {
        return sign | 0x7c00 | if fraction != 0 { 0x200 } else { 0 };
    }
    let e = exponent - 127 + 15;
    if e >= 31 {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let f = fraction | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = f >> shift;
        let round =
            (f >> (shift - 1) & 1) & (half & 1 | u32::from(f & ((1 << (shift - 1)) - 1) != 0));
        return sign | (half + round) as u16;
    }
    let half = (e as u32) << 10 | fraction >> 13;
    let round = (fraction >> 12 & 1) & (half & 1 | u32::from(fraction & 0xfff != 0));
    sign | (half + round) as u16
}
