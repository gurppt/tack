//! Sixty-four remappable local camera slots per board; never shared document commands.
use serde::{Deserialize, Serialize};
use tack_assets::AssetError;
use tack_core::Camera;
pub const MAX_VIEWS: usize = 64;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub board: String,
    pub slot: u8,
    pub center: [f64; 2],
    pub zoom: f64,
}
pub fn validate(views: &[View]) -> Result<(), AssetError> {
    if views.len() > MAX_VIEWS {
        return Err("Too many local views (maximum 64)".into());
    }
    for (i, view) in views.iter().enumerate() {
        if usize::from(view.slot) >= MAX_VIEWS
            || view.board.len() != 32
            || !view.board.bytes().all(|b| b.is_ascii_hexdigit())
            || views[..i]
                .iter()
                .any(|v| v.board == view.board && v.slot == view.slot)
        {
            return Err("Invalid local camera slot".into());
        }
        Camera::new([1, 1]).set_view(view.center, view.zoom)?;
    }
    Ok(())
}
pub fn assign(views: &mut Vec<View>, view: View) -> Result<(), AssetError> {
    validate(std::slice::from_ref(&view))?;
    if let Some(old) = views
        .iter_mut()
        .find(|v| v.board == view.board && v.slot == view.slot)
    {
        *old = view;
    } else if views.len() < MAX_VIEWS {
        views.push(view);
    } else {
        return Err("Too many local views (maximum 64)".into());
    }
    Ok(())
}
pub fn merge(base: &[View], desired: &[View], current: &[View]) -> Result<Vec<View>, AssetError> {
    let mut next = current.to_vec();
    for view in desired {
        let old = base
            .iter()
            .find(|v| v.board == view.board && v.slot == view.slot);
        if old == Some(view) {
            continue;
        }
        let present = current
            .iter()
            .find(|v| v.board == view.board && v.slot == view.slot);
        if present != old && present != Some(view) {
            return Err("Camera slot changed in another window".into());
        }
        assign(&mut next, view.clone())?;
    }
    validate(&next)?;
    Ok(next)
}
