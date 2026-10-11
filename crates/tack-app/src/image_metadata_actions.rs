//! Image and lock changes share the ordinary atomic history/LAN boundary.
use crate::{actions::Action, image_input::ImageInput};
use tack_assets::AssetError;
use tack_core::*;
impl ImageInput {
    pub(crate) fn image_metadata_action(
        &mut self,
        action: Action,
        editor: &mut DocumentEditor,
    ) -> Result<(), AssetError> {
        self.images.cancel();
        if action == Action::SelectLockedObjects {
            self.images.selection.clear();
            for o in editor.document().objects().filter(|o| o.locked()) {
                self.images.selection.select(Some(o.id()), true);
            }
            self.status = "Locked objects selected; Toggle Lock Selection to unlock".into();
            return Ok(());
        }
        let doc = editor.document();
        let lock = !self
            .images
            .selection
            .ids()
            .all(|id| doc.object(id).is_some_and(|o| o.locked()));
        let grayscale = !self
            .images
            .selection
            .ids()
            .filter_map(|id| doc.object_render_data(id))
            .all(|d| d.grayscale);
        let mut edits = Vec::new();
        for id in self.images.selection.ids() {
            let Some(o) = doc.object(id) else {
                continue;
            };
            if action == Action::ToggleLockSelection {
                edits.push(Command::SetLocked {
                    object: id,
                    locked: lock,
                });
                continue;
            }
            if o.locked() {
                continue;
            }
            let Some(d) = doc.object_render_data(id) else {
                continue;
            };
            match action {
                Action::ResetCrop => edits.push(Command::SetCrop {
                    object: id,
                    crop: Crop::FULL,
                }),
                Action::ToggleGrayscale => edits.push(Command::SetGrayscale {
                    object: id,
                    grayscale,
                }),
                Action::ActualSize => {
                    let pixels = doc
                        .asset(d.asset_id)
                        .ok_or("Missing image asset")?
                        .pixel_size();
                    let crop = d.crop.uv_rect();
                    let size = [
                        f64::from(pixels[0]) * crop[2],
                        f64::from(pixels[1]) * crop[3],
                    ];
                    edits.push(Command::SetTransform {
                        object: id,
                        transform: Transform::new(
                            d.transform.center(),
                            size,
                            d.transform.rotation(),
                            d.transform.flips(),
                        )?,
                    });
                }
                _ => {}
            }
        }
        editor.execute(Command::Batch(edits))?;
        self.status = action.label().into();
        Ok(())
    }
}
