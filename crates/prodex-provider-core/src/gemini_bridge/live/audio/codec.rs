//! Gemini Live G.711 audio codec helpers.

pub fn gemini_provider_core_live_decode_ulaw(byte: u8) -> i16 {
    let value = !byte;
    let sign = value & 0x80;
    let exponent = (value >> 4) & 0x07;
    let mantissa = value & 0x0f;
    let sample = ((((mantissa as i32) << 3) + 0x84) << exponent) - 0x84;
    if sign != 0 {
        -(sample as i16)
    } else {
        sample as i16
    }
}

pub fn gemini_provider_core_live_decode_alaw(byte: u8) -> i16 {
    let value = byte ^ 0x55;
    let sign = value & 0x80;
    let exponent = (value & 0x70) >> 4;
    let mantissa = value & 0x0f;
    let sample = if exponent == 0 {
        ((mantissa as i32) << 4) + 8
    } else {
        (((mantissa as i32) << 4) + 0x108) << (exponent - 1)
    };
    if sign != 0 {
        sample as i16
    } else {
        -(sample as i16)
    }
}
