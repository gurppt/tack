//! Scalar-safe deterministic plain-note wrapping, without allocation or font I/O.
use crate::pixel_font;
pub fn advance(c: char, size: f64) -> f64 {
    if c == '\t' {
        return size * 0.6015625 * 4.;
    }
    if tack_render::note_glyph_uv(c).is_some() {
        size * 0.6015625
    } else {
        pixel_font::glyph(c).1 as f64 / 16. * size
    }
}
pub struct NoteLines<'a> {
    text: &'a str,
    width: f64,
    size: f64,
}
impl<'a> NoteLines<'a> {
    pub fn new(text: &'a str, width: f64, size: f64) -> Self {
        Self {
            text,
            width: width.max(1e-6),
            size,
        }
    }
}
impl<'a> Iterator for NoteLines<'a> {
    type Item = (&'a str, f64);
    fn next(&mut self) -> Option<Self::Item> {
        if self.text.is_empty() {
            return None;
        }
        let mut width = 0.;
        let mut end = self.text.len();
        let mut consume = end;
        for (i, c) in self.text.char_indices() {
            if c == '\n' {
                end = i;
                consume = i + 1;
                break;
            }
            let next = advance(c, self.size);
            if width + next > self.width && i > 0 {
                end = i;
                consume = i;
                break;
            }
            width += next;
        }
        let line = &self.text[..end];
        self.text = &self.text[consume..];
        Some((line, width))
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn wraps_unicode_and_tabs_without_splitting_bytes_or_losing_scalars() {
        let input = "café猫😀\nЖ\tend";
        let lines: Vec<_> = super::NoteLines::new(input, 30., 16.).collect();
        assert_eq!(
            lines.iter().map(|(s, _)| *s).collect::<String>(),
            input.replace('\n', "")
        );
        assert!(lines.len() > 3);
        assert!(super::NoteLines::new("", 20., 16.).next().is_none());
        assert_eq!(
            super::NoteLines::new("a\n\nb", 20., 16.)
                .collect::<Vec<_>>()
                .len(),
            3
        );
    }
}
