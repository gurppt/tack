//! Reproducible native gesture driver through production semantic dispatch.
use crate::{
    actions::{Action, ActionEvent, ActionPhase, HoldToken},
    image_input::ImageInput,
};
use tack_assets::AssetError;
use tack_core::{Camera, DocumentEditor, DocumentQuery, ImageRenderData};
pub const SCENARIOS: [&str; 10] = [
    "drag",
    "resize",
    "rotate",
    "crop",
    "multi10",
    "multi100",
    "cancel",
    "snap",
    "grid-hidden",
    "grid-visible",
];
pub struct ImageBenchmark {
    name: String,
    frame: usize,
    start: [f64; 2],
    original: Vec<ImageRenderData>,
    pub samples: Vec<f64>,
    pub commits: usize,
    pub cancels: usize,
    pub invariants: bool,
    pub snap_queries: Vec<f64>,
}
impl ImageBenchmark {
    pub fn new(name: String) -> Result<Self, AssetError> {
        if !SCENARIOS.contains(&name.as_str()) {
            return Err("unknown interaction scenario".into());
        }
        Ok(Self {
            name,
            frame: 0,
            start: [0.; 2],
            original: Vec::new(),
            samples: Vec::with_capacity(7200),
            commits: 0,
            cancels: 0,
            invariants: true,
            snap_queries: Vec::new(),
        })
    }
    pub fn drive(
        &mut self,
        input: &mut ImageInput,
        editor: &mut DocumentEditor,
        camera: &mut Camera,
    ) -> Result<(), AssetError> {
        let started = std::time::Instant::now();
        if self.name.starts_with("grid-") {
            input.grid_visible = self.name == "grid-visible";
            self.frame += 1;
            if self.samples.len() < 7200 {
                self.samples.push(started.elapsed().as_secs_f64() * 1000.);
            }
            return Ok(());
        }
        if self.name == "snap" {
            input.snap.enabled = true;
            input.snap.grid = true;
            input.snap.measure = true;
        }
        input.snap.last_query_ms = None;
        let phase = self.frame % 61;
        let token = HoldToken(1_000_000 + self.frame as u64 / 61);
        if phase == 0 {
            input.cancel();
            input.images.selection.clear();
            let count = match self.name.as_str() {
                "multi10" => 10,
                "multi100" => 100,
                _ => 1,
            };
            for id in editor.document().object_order().iter().take(count) {
                input.images.selection.select(Some(*id), true);
            }
            self.original = input
                .images
                .selection
                .ids()
                .filter_map(|id| editor.document().object_render_data(id))
                .collect();
            let first = self.original.first().ok_or("benchmark needs images")?;
            let frame = input
                .images
                .frame(editor.document())
                .ok_or("selection frame")?;
            let world = match self.name.as_str() {
                "resize" => input.gizmo.handle(frame, camera, 4),
                "crop" => input.gizmo.handle(frame, camera, 3),
                "rotate" => [
                    first.transform.center()[0] + first.transform.size()[0] * 0.4,
                    first.transform.center()[1],
                ],
                _ => first.transform.center(),
            };
            self.start = camera.world_to_screen(world);
            input.cursor_moved(self.start, editor, camera)?;
            input.images.crop_mode = self.name == "crop";
            let action = if self.name == "rotate" {
                Action::RotateImage
            } else {
                Action::ImagePointer
            };
            input.dispatch(
                ActionEvent {
                    action,
                    phase: ActionPhase::Begin(token),
                },
                editor,
                camera,
            )?;
        } else if phase < 60 {
            let fraction = phase as f64 / 59.;
            let first = self.original.first().ok_or("benchmark record")?;
            let size = first.transform.size().map(|s| s * camera.zoom());
            let next = match self.name.as_str() {
                "resize" => [
                    self.start[0] + size[0] * (fraction * 2. - 0.6),
                    self.start[1] + size[1] * (fraction * 2. - 0.6),
                ],
                "crop" => [self.start[0] - size[0] * 0.3 * fraction, self.start[1]],
                "rotate" => {
                    let center = camera.world_to_screen(first.transform.center());
                    let angle = fraction * 2.;
                    [
                        center[0] + size[0] * 0.4 * angle.cos(),
                        center[1] + size[0] * 0.4 * angle.sin(),
                    ]
                }
                _ => [
                    self.start[0] + fraction * 160.,
                    self.start[1] + (fraction * 5.).sin() * 60.,
                ],
            };
            input.cursor_moved(next, editor, camera)?;
        } else {
            let before = editor.undo_len();
            if self.name == "cancel" {
                if self.cancels.is_multiple_of(2) {
                    input.dispatch(
                        ActionEvent {
                            action: Action::CancelInteraction,
                            phase: ActionPhase::Invoke,
                        },
                        editor,
                        camera,
                    )?;
                } else {
                    input.physical(crate::input::PhysicalEvent::FocusLost, editor, camera)?;
                }
                self.cancels += 1;
            } else {
                input.dispatch(
                    ActionEvent {
                        action: Action::ImagePointer,
                        phase: ActionPhase::End(token),
                    },
                    editor,
                    camera,
                )?;
                self.invariants &= editor.undo_len() == before + 1;
                self.commits += 1;
                editor.undo()?;
            }
            self.invariants &= self
                .original
                .iter()
                .all(|d| editor.document().object_render_data(d.object_id) == Some(*d));
            if !self.invariants {
                return Err("native gesture history/cancellation invariant failed".into());
            }
        }
        if let Some(ms) = input.snap.last_query_ms
            && self.snap_queries.len() < 7200
        {
            self.snap_queries.push(ms);
        }
        self.frame += 1;
        if self.samples.len() < 7200 {
            self.samples.push(started.elapsed().as_secs_f64() * 1000.);
        }
        Ok(())
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}
