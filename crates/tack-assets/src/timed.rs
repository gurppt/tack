//! Codec-independent timed-asset seam. Static boards never construct a backend or clock.
//! Future SWF: passive main timeline/Graphic/static MovieClip only; no VM/network/interaction.
use crate::{AssetError, Decoded};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimedProbe {
    pub dimensions: [u32; 2],
    pub frame_count: u32,
}
pub trait TimedAssetBackend {
    fn probe(&self) -> TimedProbe;
    fn poster_frame(&mut self) -> Result<Decoded, AssetError>;
    fn frame_count(&self) -> u32 {
        self.probe().frame_count
    }
    fn current_frame(&self) -> u32;
    fn seek(&mut self, frame: u32) -> Result<Decoded, AssetError>;
    fn scrub_begin(&mut self) -> Result<(), AssetError>;
    fn scrub_to(&mut self, frame: u32) -> Result<Decoded, AssetError>;
    fn scrub_end(&mut self) -> Result<(), AssetError>;
    fn play(&mut self) -> Result<(), AssetError>;
    fn pause(&mut self) -> Result<(), AssetError>;
    fn close(&mut self) -> Result<(), AssetError>;
}
/// Reserved contextual Numpad semantics: normal one frame, Shift ten. Caller
/// invokes this only with an active timed asset; static annotation adjustment stays unchanged.
pub fn adjusted_frame(current: u32, count: u32, increase: bool, shift: bool) -> u32 {
    let step = if shift { 10 } else { 1 };
    if increase {
        current.saturating_add(step).min(count.saturating_sub(1))
    } else {
        current.saturating_sub(step)
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn contextual_frames_are_bounded() {
        assert_eq!(super::adjusted_frame(3, 20, true, true), 13);
        assert_eq!(super::adjusted_frame(3, 20, false, true), 0);
        assert_eq!(super::adjusted_frame(18, 20, true, true), 19);
        assert_eq!(super::adjusted_frame(0, 0, true, false), 0);
    }
}
