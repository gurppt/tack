//! Tack Label Bitmap (OFL-1.1); no decoder/atlas, only visible read-only rows.
static FONT: [u8; 4_256_961] = *include_bytes!("../../../assets/pixel-font/tack-label-glyphs.bin");
const RECORD: usize = 37;
pub fn glyph(character: char) -> ([u32; 8], usize) {
    let count = FONT.len() / RECORD;
    let code = |i: usize| {
        let p = i * RECORD;
        u32::from_le_bytes([FONT[p], FONT[p + 1], FONT[p + 2], FONT[p + 3]])
    };
    let mut lo = 0;
    let mut hi = count;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if code(mid) < character as u32 {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo >= count || code(lo) != character as u32 {
        return if character == '\u{fffd}' {
            ([0; 8], 8)
        } else {
            glyph('\u{fffd}')
        };
    }
    let p = lo * RECORD;
    let bits = std::array::from_fn(|i| {
        let a = p + 5 + i * 4;
        u32::from(u16::from_be_bytes([FONT[a], FONT[a + 1]]))
            | (u32::from(u16::from_be_bytes([FONT[a + 2], FONT[a + 3]])) << 16)
    });
    (bits, FONT[p + 4] as usize)
}
#[cfg(test)]
mod tests {
    #[test]
    fn unicode_rows_are_distinct_and_font_sorted() {
        assert_eq!(super::FONT.len() % super::RECORD, 0);
        let mut previous = None;
        for r in super::FONT.chunks_exact(super::RECORD) {
            let id = u32::from_le_bytes([r[0], r[1], r[2], r[3]]);
            assert!(previous.is_none_or(|old| old < id));
            previous = Some(id);
            assert!([8, 16].contains(&r[4]));
        }
        assert_ne!(super::glyph('é').0, super::glyph('e').0);
        assert_ne!(super::glyph('猫').0, super::glyph('é').0);
        assert_ne!(super::glyph('Ж').0, super::glyph('猫').0);
        assert_ne!(super::glyph('😀').0, super::glyph('Ж').0);
    }
}
