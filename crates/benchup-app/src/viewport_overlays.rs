//! Model-space descriptions shared by viewport hosts, before camera projection.
use super::*;

#[derive(Clone, PartialEq)]
pub(crate) struct Segment {
    pub a: DVec3,
    pub b: DVec3,
    pub color: Color32,
    pub width: f32,
    pub dashed: bool,
}

#[cfg(all(feature = "openxr", not(target_arch = "wasm32")))]
#[derive(Clone, PartialEq)]
pub(crate) struct Label {
    pub point: DVec3,
    pub text: String,
    pub color: Color32,
}

#[cfg(all(feature = "openxr", not(target_arch = "wasm32")))]
#[derive(Default)]
pub(crate) struct Overlays {
    pub segments: Vec<Segment>,
    pub markers: Vec<(DVec3, Color32)>,
    pub ticks: Vec<(DVec3, DVec3, Color32)>,
    pub labels: Vec<Label>,
    pub surfaces: Vec<Triangle>,
}

#[cfg(all(feature = "openxr", not(target_arch = "wasm32")))]
#[derive(Clone, PartialEq)]
pub(crate) struct Triangle {
    pub points: [DVec3; 3],
    pub color: Color32,
}

impl BenchUpApp {
    pub(super) fn drawing_preview_segments(&self, colors: Palette) -> Vec<Segment> {
        let mut lines = Vec::new();
        if let (Some(start), Some(snap)) = (self.trace_start(), self.line_preview_snap()) {
            lines.push(Segment {
                a: start,
                b: snap.point,
                width: 2.2,
                dashed: false,
                color: self
                    .preview_alignment()
                    .or_else(|| self.active_drawing_guide().map(|g| g.axis))
                    .map(|axis| self.axis_color(axis))
                    .unwrap_or(colors.ink),
            });
        }
        lines.extend(self.drawing_points().windows(2).map(|pair| {
            Segment {
                a: pair[0],
                b: pair[1],
                width: 2.2,
                dashed: false,
                color: self
                    .lock
                    .map(|axis| self.axis_color(axis))
                    .unwrap_or(ACCENT),
            }
        }));
        lines
    }

    #[cfg(all(feature = "openxr", not(target_arch = "wasm32")))]
    pub(super) fn spatial_tool_overlays(&self, colors: Palette) -> Overlays {
        let mut result = Overlays::default();
        if self.tool == Tool::Flip {
            for (axis, quad) in self.flip_planes() {
                let color = self.axis_color(axis);
                let fill = Color32::from_rgba_unmultiplied(
                    color.r(),
                    color.g(),
                    color.b(),
                    if self.flip_hover == Some(axis) {
                        65
                    } else {
                        24
                    },
                );
                for points in [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]] {
                    result.surfaces.push(Triangle {
                        points,
                        color: fill,
                    });
                }
                for i in 0..4 {
                    result.segments.push(Segment {
                        a: quad[i],
                        b: quad[(i + 1) % 4],
                        color,
                        width: 1.5,
                        dashed: false,
                    });
                }
                result.labels.push(Label {
                    point: quad[2],
                    text: format!("Flip {}", axis.label()),
                    color,
                });
            }
        }
        if self.history_preview.is_some() {
            return result;
        }
        result.segments = self.drawing_preview_segments(colors);
        if self.doc.display.guides {
            let extent = self.doc.camera.distance.max(1.0) * 5.0;
            for guide in self.active_drawing_axis_guides() {
                let delta = guide.axis.vector() * extent;
                result.segments.push(Segment {
                    a: guide.origin - delta,
                    b: guide.origin + delta,
                    width: 1.0,
                    color: self.axis_color(guide.axis),
                    dashed: true,
                });
            }
            if let Some((a, b)) = self.active_drawing_projection() {
                result.segments.push(Segment {
                    a,
                    b,
                    width: 1.2,
                    color: self.inference_color(),
                    dashed: true,
                });
            }
        }
        if self.tool.is_drawing()
            || matches!(self.tool, Tool::Text | Tool::GrainDirection)
            || self.modeling.is_some()
        {
            if let Some(start) = self
                .trace_start()
                .or_else(|| self.drawing.as_ref().map(|drawing| drawing.origin))
            {
                result.markers.push((start, colors.ink));
            }
            let snap = if self.modeling.is_some() {
                self.modeling_preview_snap()
            } else {
                self.line_preview_snap()
            };
            if let Some(snap) = snap {
                let color = match snap.kind {
                    Inference::Axis(axis) | Inference::OnAxis(axis) => self.axis_color(axis),
                    Inference::Midpoint => {
                        let rgb = self.doc.display.midpoint_color;
                        Color32::from_rgb(rgb[0], rgb[1], rgb[2])
                    }
                    _ => self.inference_color(),
                };
                result.markers.push((snap.point, color));
                let text = if let Some(normal) = self.plane_lock() {
                    format!(
                        "{} plane locked",
                        match normal {
                            Axis::X => "YZ",
                            Axis::Y => "XZ",
                            Axis::Z => "XY",
                        }
                    )
                } else if let Some(axis) = self.lock {
                    format!("{} locked", axis.label())
                } else {
                    snap.kind.label().to_owned()
                };
                result.labels.push(Label {
                    point: snap.point,
                    text,
                    color,
                });
                if let Some(start) = self.line_start {
                    result.labels.push(Label {
                        point: (start + snap.point) * 0.5,
                        text: self
                            .doc
                            .display
                            .units
                            .format_length(start.distance(snap.point)),
                        color: colors.ink,
                    });
                }
            }
        }
        if self.doc.display.dimensions && self.tool.is_measuring() && self.tool != Tool::Text {
            let points = self.measurement_points();
            result
                .markers
                .extend(points.iter().map(|point| (*point, colors.green)));
            if self.tool == Tool::Dimensions {
                if let Some(dimension) = self.dimension_preview() {
                    let points = dimension.layout(&self.doc.model);
                    for (a, b) in [(0, 2), (1, 3), (2, 3)] {
                        result.segments.push(Segment {
                            a: points[a],
                            b: points[b],
                            color: colors.green,
                            width: 1.5,
                            dashed: false,
                        });
                    }
                    for (a, b) in [(2, 3), (3, 2)] {
                        result.ticks.push((points[a], points[b], colors.green));
                    }
                    result.labels.push(Label {
                        point: (points[2] + points[3]) * 0.5,
                        text: self
                            .doc
                            .display
                            .units
                            .format_length(dimension.length(&self.doc.model)),
                        color: colors.green,
                    });
                }
            } else {
                if points.len() >= 2 {
                    result.segments.push(Segment {
                        a: points[0],
                        b: points[1],
                        color: colors.green,
                        width: 1.5,
                        dashed: false,
                    });
                }
                let anchor = if self.tool == Tool::Protractor && points.len() == 3 {
                    result.segments.push(Segment {
                        a: points[0],
                        b: points[2],
                        color: colors.green,
                        width: 1.5,
                        dashed: false,
                    });
                    let arc = benchup_core::angle_arc(points[0], points[1], points[2]);
                    result.segments.extend(arc.windows(2).map(|pair| Segment {
                        a: pair[0],
                        b: pair[1],
                        color: colors.green,
                        width: 1.5,
                        dashed: false,
                    }));
                    arc.get(arc.len() / 2).copied()
                } else if points.len() == 2 {
                    Some((points[0] + points[1]) * 0.5)
                } else {
                    None
                };
                if let (Some(point), Some(text)) = (anchor, self.measurement_value()) {
                    result.labels.push(Label {
                        point,
                        text,
                        color: colors.green,
                    });
                }
            }
        }
        if let Some(edit) = &self.modeling {
            result.markers.push((edit.anchor, colors.ink));
            let end = if matches!(self.tool, Tool::Move | Tool::Scale | Tool::Rotate) {
                edit.anchor + edit.direction
            } else {
                edit.anchor + edit.direction * edit.value
            };
            result.segments.push(Segment {
                a: edit.anchor,
                b: end,
                color: self.inference_color(),
                width: 1.5,
                dashed: false,
            });
            if self.doc.display.guides {
                for guide in &edit.guides {
                    result.segments.push(Segment {
                        a: guide.origin,
                        b: end,
                        color: self.axis_color(guide.axis),
                        width: 1.5,
                        dashed: true,
                    });
                }
            }
            let value = precision::modeling_value_with_units(
                self.tool,
                &self.length_input,
                self.doc.display.units,
            )
            .unwrap_or(edit.value);
            let text = edit.error.map(str::to_owned).unwrap_or_else(|| {
                if self.tool == Tool::Rotate {
                    self.doc.display.units.format_angle(value)
                } else if self.tool == Tool::Scale {
                    format!("{value:.3}×")
                } else {
                    self.doc.display.units.format_length(value)
                }
            });
            result.labels.push(Label {
                point: end,
                text,
                color: if edit.error.is_some() {
                    colors.accent
                } else {
                    colors.ink
                },
            });
        }
        result
    }
}
