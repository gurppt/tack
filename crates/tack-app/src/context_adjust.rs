//! Bounded contextual edits use the ordinary document command/history boundary.
use crate::{annotation_input::next_style, annotation_tool::StyleAction, image_input::ImageInput};
use tack_assets::AssetError;
use tack_core::*;
pub(crate) fn note_style(s: AnnotationStyle) -> AnnotationStyle {
    AnnotationStyle::new(
        s.stroke(),
        Some(s.fill().unwrap_or(s.stroke())),
        s.width(),
        s.opacity(),
    )
    .unwrap_or(s)
}
pub(crate) fn text_size(size: f64, increase: bool) -> f64 {
    // Bitmap font steps are integral multiples; arbitrary imported/scaled sizes
    // snap in the requested direction, rather than accumulating fractions.
    let step = size / 16.;
    let next = if increase {
        step.floor() + 1.
    } else {
        step.ceil() - 1.
    };
    (next * 16.).clamp(16., 256.)
}
impl ImageInput {
    pub(crate) fn context_adjust(
        &mut self,
        increase: bool,
        coarse: bool,
        editor: &mut DocumentEditor,
    ) -> Result<(), AssetError> {
        let mut commands = Vec::new();
        self.status = "No contextual adjustment".into();
        for object in self.images.selection.ids() {
            let Some(o) = editor.document().object(object) else {
                continue;
            };
            if o.locked() {
                continue;
            }
            let ObjectKind::Annotation(a) = o.kind() else {
                continue;
            };
            if let AnnotationKind::Text(t) = a.kind() {
                let mut size = t.font_size();
                for _ in 0..if coarse { 10 } else { 1 } {
                    size = text_size(size, increase);
                }
                commands.push(Command::SetText {
                    object,
                    text: TextObject::new(t.value().to_owned(), size, t.alignment())?,
                });
                self.status = format!("Text size {} px", size as u16);
            } else {
                let style = if self.adjust_fill == Some(object)
                    && matches!(a.kind(), AnnotationKind::Rect)
                {
                    let s = a.style();
                    let alpha = s.fill().map_or(0, |c| c.0[3]);
                    let levels = [0, 64, 128, 191, 255];
                    let alpha = if increase {
                        levels.into_iter().find(|v| *v > alpha).unwrap_or(255)
                    } else {
                        levels.into_iter().rev().find(|v| *v < alpha).unwrap_or(0)
                    };
                    self.status =
                        format!("Fill {}%", (f64::from(alpha) * 100. / 255.).round() as u16);
                    let c = s.stroke().0;
                    AnnotationStyle::new(
                        s.stroke(),
                        (alpha != 0).then_some(Color([c[0], c[1], c[2], alpha])),
                        s.width(),
                        s.opacity(),
                    )?
                } else {
                    let mut s = a.style();
                    for _ in 0..if coarse { 10 } else { 1 } {
                        s = next_style(
                            s,
                            if increase {
                                StyleAction::Wider
                            } else {
                                StyleAction::Narrower
                            },
                        )?;
                    }
                    self.status = format!("Stroke {} px", s.width() as u16);
                    s
                };
                commands.push(Command::SetAnnotationStyle { object, style });
            }
        }
        editor.execute(Command::Batch(commands))?;
        Ok(())
    }
}
