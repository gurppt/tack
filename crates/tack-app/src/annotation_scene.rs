//! Source-free annotation packets and ordered ranges for the common canvas.
use crate::{
    annotation_geometry as geometry,
    annotation_tool::{AnnotationInput, Creation, NoteEdit},
    image_geometry,
    image_interaction::ImageInteraction,
};
use tack_core::*;
use tack_render::{AnnotationPrimitive, CanvasDraw, DrawProductImage, MAX_ANNOTATION_PRIMITIVES};

#[derive(Default)]
pub struct AnnotationScene {
    pub caret_visible: bool,
    pub primitives: Vec<AnnotationPrimitive>,
    pub palette: crate::ui_theme::Palette,
    pub order: Vec<CanvasDraw>,
    ranges: Vec<(ObjectId, usize, usize)>,
    pub omitted: usize,
    pub glyphs: usize,
    pub layout_ms: f64,
    pub build_ms: f64,
}
pub(crate) fn primitive(
    points: [[f64; 2]; 4],
    size: [f64; 2],
    kind: u32,
    style: AnnotationStyle,
) -> AnnotationPrimitive {
    AnnotationPrimitive {
        points,
        size,
        kind,
        width: style.width(),
        opacity: style.opacity().value() as f32,
        stroke: style.stroke().rgba(if kind == 1 {
            Opacity::OPAQUE
        } else {
            style.opacity()
        }),
        fill: style.fill().map_or([0.; 4], |c| {
            c.rgba(if kind == 1 {
                Opacity::OPAQUE
            } else {
                style.opacity()
            })
        }),
        mapping: [0.; 4],
        bitmap: [0; 8],
    }
}
impl AnnotationScene {
    fn presentation_style(&self, style: AnnotationStyle) -> AnnotationStyle {
        if style.fill().is_some() {
            return style;
        }
        let luminance = |c: [f32; 4]| c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722;
        let text = luminance(style.stroke().rgba(Opacity::OPAQUE));
        let background = luminance(self.palette.background);
        let contrast = (text.max(background) + 0.05) / (text.min(background) + 0.05);
        if contrast >= 3. {
            return style;
        }
        // A flat contrast backing is presentation only; authored colors remain intact.
        let backing = if text > 0.18 {
            Color([6, 8, 10, 255])
        } else {
            Color([220, 224, 228, 255])
        };
        AnnotationStyle::new(
            style.stroke(),
            Some(backing),
            style.width(),
            style.opacity(),
        )
        .unwrap_or(style)
    }
    fn editor_style(&self, style: AnnotationStyle) -> AnnotationStyle {
        style
    }

    pub(crate) fn push(&mut self, p: AnnotationPrimitive) -> bool {
        if self.primitives.len() == MAX_ANNOTATION_PRIMITIVES {
            return false;
        }
        self.primitives.push(p);
        true
    }
    fn shape(&mut self, t: Transform, camera: &Camera, style: AnnotationStyle) -> bool {
        let size = t.size();
        let width = (style.width() * camera.zoom() / camera.ui_scale())
            .round()
            .max(1.)
            * camera.ui_scale()
            / camera.zoom();
        let margin = width / 2. + camera.ui_scale() / camera.zoom();
        let points = [[-1., -1.], [-1., 1.], [1., -1.], [1., 1.]].map(|d| {
            image_geometry::world(
                t,
                [
                    d[0] * (size[0] / 2. + margin),
                    d[1] * (size[1] / 2. + margin),
                ],
            )
        });
        self.push(primitive(points, size, 1, style))
    }
    pub fn segment(
        &mut self,
        start: [f64; 2],
        end: [f64; 2],
        style: AnnotationStyle,
        camera: &Camera,
    ) -> bool {
        let length = (end[0] - start[0]).hypot(end[1] - start[1]);
        let width = (style.width() * camera.zoom() / camera.ui_scale())
            .round()
            .max(1.)
            * camera.ui_scale()
            / camera.zoom();
        let margin = width / 2. + camera.ui_scale() / camera.zoom();
        let v = if length > 0. {
            [(end[0] - start[0]) / length, (end[1] - start[1]) / length]
        } else {
            [1., 0.]
        };
        let center = [(start[0] + end[0]) / 2., (start[1] + end[1]) / 2.];
        let points = [[-1., -1.], [-1., 1.], [1., -1.], [1., 1.]].map(|d| {
            [
                center[0] + d[0] * v[0] * (length / 2. + margin) - d[1] * v[1] * margin,
                center[1] + d[0] * v[1] * (length / 2. + margin) + d[1] * v[0] * margin,
            ]
        });
        let mut p = primitive(points, [length, 0.], 3, style);
        p.fill = [0.; 4];
        self.push(p)
    }
    fn joined_segment(
        &mut self,
        start: [f64; 2],
        end: [f64; 2],
        previous: Option<[f64; 2]>,
        style: AnnotationStyle,
        camera: &Camera,
    ) -> bool {
        if !self.segment(start, end, style, camera) {
            return false;
        }
        if let Some(previous) = previous
            && let Some(p) = self.primitives.last_mut()
        {
            let a = camera.world_to_screen(previous);
            let b = camera.world_to_screen(start);
            p.mapping = [a[0] as f32, a[1] as f32, b[0] as f32, b[1] as f32];
            p.bitmap[0] = 1;
        }
        true
    }
    fn arrow(
        &mut self,
        start: [f64; 2],
        end: [f64; 2],
        style: AnnotationStyle,
        camera: &Camera,
    ) -> bool {
        if !self.segment(start, end, style, camera) {
            return false;
        }
        let [tip, a, b] = geometry::arrow_head(start, end, style.width());
        let mut p = primitive(
            [tip, a, b, [a[0] + b[0] - tip[0], a[1] + b[1] - tip[1]]],
            [1.; 2],
            4,
            style,
        );
        let a = camera.world_to_screen(start);
        let b = camera.world_to_screen(end);
        p.mapping = [a[0] as f32, a[1] as f32, b[0] as f32, b[1] as f32];
        p.bitmap[0] = 1;
        self.push(p)
    }
    fn annotation(
        &mut self,
        a: &Annotation,
        t: Transform,
        camera: &Camera,
        edit: Option<&NoteEdit>,
        style: AnnotationStyle,
        note_size: Option<f64>,
    ) -> bool {
        match a.kind() {
            AnnotationKind::Rect => self.shape(t, camera, style),
            AnnotationKind::Line(l) => {
                let [start, end] = l.points().map(|p| geometry::point(t, p));
                self.segment(start, end, style, camera)
            }
            AnnotationKind::Arrow(l) => {
                let [start, end] = l.points().map(|p| geometry::point(t, p));
                self.arrow(start, end, style, camera)
            }
            AnnotationKind::Scribble(s) => s.points().windows(2).enumerate().all(|(i, p)| {
                self.joined_segment(
                    geometry::point(t, p[0]),
                    geometry::point(t, p[1]),
                    i.checked_sub(1).map(|i| geometry::point(t, s.points()[i])),
                    style,
                    camera,
                )
            }),
            AnnotationKind::Text(text) => {
                let start = std::time::Instant::now();
                let result = self.note(
                    if edit.is_some() {
                        self.editor_style(style)
                    } else {
                        self.presentation_style(style)
                    },
                    t,
                    (
                        edit.map_or(text.value(), |e| e.value.as_str()),
                        edit.map(|_| self.caret_visible),
                    ),
                    edit.map_or(note_size.unwrap_or(text.font_size()), |e| e.size),
                    edit.map_or(text.alignment(), |e| e.alignment),
                );
                self.layout_ms += start.elapsed().as_secs_f64() * 1000.;
                result
            }
        }
    }
    pub fn build(
        &mut self,
        doc: &Document,
        images: &ImageInteraction,
        input: &AnnotationInput,
        camera: &Camera,
        draws: &[DrawProductImage],
    ) {
        let start = std::time::Instant::now();
        self.primitives.clear();
        self.order.clear();
        self.ranges.clear();
        self.omitted = 0;
        self.glyphs = 0;
        self.layout_ms = 0.;
        let viewport = camera.viewport();
        let padding = 2. / camera.zoom();
        let view = WorldRect {
            x: viewport.x - padding,
            y: viewport.y - padding,
            width: viewport.width + 2. * padding,
            height: viewport.height + 2. * padding,
        };
        let first = self.primitives.len();
        if let Some(c) = &input.creation
            && !self.creation(c, camera)
        {
            self.primitives.truncate(first);
            self.omitted += 1;
        }
        if let Some(e) = &input.edit
            && e.is_new
            && !self.note(
                self.editor_style(e.style),
                e.transform,
                (&e.value, Some(self.caret_visible)),
                e.size,
                e.alignment,
            )
        {
            self.primitives.truncate(first);
            self.omitted += 1;
        }
        let transient_end = self.primitives.len();
        // Selected/editing objects get budget first; actual draws still follow document order.
        for selected in [true, false] {
            for id in doc.object_order() {
                if images.selection.contains(*id) != selected {
                    continue;
                }
                let Some(o) = doc.object(*id) else {
                    continue;
                };
                let ObjectKind::Annotation(a) = o.kind() else {
                    continue;
                };
                let t = images.preview_transform(doc, *id).unwrap_or(o.transform());
                if !a.bounds(t).is_ok_and(|b| b.intersects(view)) {
                    continue;
                }
                let first = self.primitives.len();
                let glyphs = self.glyphs;
                if self.annotation(
                    a,
                    t,
                    camera,
                    input.edit.as_deref().filter(|e| !e.is_new && e.id == *id),
                    images
                        .preview_opacity(*id)
                        .and_then(|opacity| {
                            AnnotationStyle::new(
                                images.preview_style(*id).unwrap_or(a.style()).stroke(),
                                images.preview_style(*id).unwrap_or(a.style()).fill(),
                                images.preview_style(*id).unwrap_or(a.style()).width(),
                                opacity,
                            )
                            .ok()
                        })
                        .unwrap_or(a.style()),
                    images.preview_note_size(*id),
                ) {
                    if first < self.primitives.len() {
                        self.ranges.push((*id, first, self.primitives.len()));
                    }
                } else {
                    self.primitives.truncate(first);
                    self.glyphs = glyphs;
                    self.omitted += 1;
                }
            }
        }
        self.ranges.sort_unstable_by_key(|r| r.0);
        let mut image = 0;
        for id in doc.object_order() {
            if image < draws.len() && draws[image].data.object_id == *id {
                while image < draws.len() && draws[image].data.object_id == *id {
                    self.order.push(CanvasDraw::Image(image));
                    image += 1;
                }
            } else if let Ok(index) = self.ranges.binary_search_by_key(id, |r| r.0) {
                let (_, start, end) = self.ranges[index];
                self.draw_range(start, end);
            }
        }
        self.draw_range(0, transient_end);
        self.build_ms = start.elapsed().as_secs_f64() * 1000.;
    }
    fn draw_range(&mut self, start: usize, end: usize) {
        if start == end {
            return;
        }
        if let Some(CanvasDraw::Annotations { end: prior, .. }) = self.order.last_mut()
            && *prior == start
        {
            *prior = end;
        } else {
            self.order.push(CanvasDraw::Annotations { start, end });
        }
    }
    fn creation(&mut self, c: &Creation, camera: &Camera) -> bool {
        if c.tool == crate::actions::Tool::Scribble {
            return c.points.windows(2).enumerate().all(|(i, p)| {
                self.joined_segment(
                    p[0],
                    p[1],
                    i.checked_sub(1).map(|i| c.points[i]),
                    c.style,
                    camera,
                )
            });
        }
        let Ok(t) = c.transform() else {
            return true;
        };
        let Ok(kind) = c.kind(t) else {
            return true;
        };
        self.annotation(
            &Annotation::new(kind, c.style),
            t,
            camera,
            None,
            c.style,
            None,
        )
    }
}
