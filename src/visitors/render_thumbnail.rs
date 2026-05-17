use gcode::core::{
    BlockVisitor, CommandVisitor, ControlFlow, Diagnostics, HasDiagnostics, Noop, Number,
    ProgramVisitor, Span, Value,
};
use image::DynamicImage;
use log::{debug, trace};

/// Mode for interpreting coordinates
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum PositionMode {
    Absolute,
    Relative,
}

/// Tracks the current printer position and modal settings
#[derive(Debug, Copy, Clone)]
struct PrinterState {
    /// Current X position from the origin, in millimeters
    x: f32,
    /// Current Y position from the origin, in millimeters
    y: f32,
    /// Current Z position from the origin, in millimeters
    z: f32,

    /// Current extrusion position, in millimeters
    e: f32,

    /// Current mode for interpreting X/Y/Z coordinates
    distance_mode: PositionMode,
    /// Current mode for interpreting extrusion (E) values
    extrusion_mode: PositionMode,
}

impl Default for PrinterState {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            e: 0.0,

            distance_mode: PositionMode::Absolute,
            extrusion_mode: PositionMode::Absolute,
        }
    }
}

/// A point in 3D space, in millimeters from the origin.
#[derive(Debug, Copy, Clone)]
struct Point3D {
    x: f32,
    y: f32,
    z: f32,
}

/// A line segment in 3D space, representing a movement of the printer head.
#[derive(Debug, Copy, Clone)]
struct Segment3D {
    start: Point3D,
    end: Point3D,
}

/// Projects a 3D point into 2D for a 3/4 view from above.
#[inline]
fn project_point(point: Point3D) -> (f32, f32) {
    // Basic isometric projection
    const YAW: f32 = -40.0_f32.to_radians();
    const PITCH: f32 = 45.0_f32.to_radians();
    const Z_SCALE: f32 = 1.1_f32;

    // Sadly this cant be const yet
    // TODO: maybe still cache it? Not sure if it would actually be a perf
    // improvement though, try it and benchmark it before deciding
    let (sin_yaw, cos_yaw) = YAW.sin_cos();
    let (sin_pitch, cos_pitch) = PITCH.sin_cos();

    let x1 = point.x * cos_yaw - point.y * sin_yaw;
    let y1 = point.x * sin_yaw + point.y * cos_yaw;
    let z1 = point.z * Z_SCALE;

    let y2 = y1 * cos_pitch - z1 * sin_pitch;

    (x1, -y2)
}

/// Maps a 2D point in printer coordinates to SVG coordinates, given the bounding box and scale.
#[inline]
fn map_to_svg(point: (f32, f32), min_x: f32, max_y: f32, scale: f32, padding: f32) -> (f32, f32) {
    let x = (point.0 - min_x) * scale + padding;
    let y = (max_y - point.1) * scale + padding;
    (x, y)
}

/// Parses the G-code file and renders a thumbnail from it.
pub(crate) struct RenderThumbnailVisitor {
    diagnostics: Noop,
    state: PrinterState,
    segments: Vec<Segment3D>,
    ignore_priming_line: bool,
}
impl RenderThumbnailVisitor {
    pub fn new(ignore_priming_line: bool) -> Self {
        Self {
            diagnostics: Noop,
            state: PrinterState::default(),
            segments: Vec::new(),
            ignore_priming_line,
        }
    }

    pub fn render(&self) -> DynamicImage {
        /// The target size for the thumbnail, in pixels.
        /// The rendered image will be a square of this size.
        const TARGET_SIZE: u32 = 512;
        /// Padding to apply around the model in the thumbnail, in pixels.
        const PADDING: f32 = 10.0;
        /// Color for the model lines.
        const MODEL_COLOR: &str = "#ffffff";

        /// Shadow lines, to help with depth perception.
        /// They have an offset and a thicker stroke, and are slightly transparent.
        // Offset the shadow to the down-left
        const SHADOW_OFFSET_X: f32 = -4.0;
        const SHADOW_OFFSET_Y: f32 = 5.0;
        const SHADOW_STROKE_SIZE: f32 = 1.0;
        const SHADOW_OPACITY: f32 = 0.2;
        const SHADOW_COLOR: &str = "#000000";

        // Compute the bounding box of the projected print from the collected segments
        let (min_x, max_x, min_y, max_y) = self.segments.iter().fold(
            (
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
            ),
            |acc, seg| {
                let (mut min_x, mut max_x, mut min_y, mut max_y) = acc;
                for p in [seg.start, seg.end] {
                    let (px, py) = project_point(p);
                    min_x = min_x.min(px);
                    max_x = max_x.max(px);
                    min_y = min_y.min(py);
                    max_y = max_y.max(py);
                }
                (min_x, max_x, min_y, max_y)
            },
        );

        let width = (max_x - min_x).max(1.0);
        let height = (max_y - min_y).max(1.0);

        // TODO: Make that configurable
        let scale_x = (TARGET_SIZE as f32 - 2.0 * PADDING) / width;
        let scale_y = (TARGET_SIZE as f32 - 2.0 * PADDING) / height;
        let scale = scale_x.min(scale_y);

        // TODO: Not sure String is the best fit here
        let mut svg_out = String::new();
        svg_out.push_str(&format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{0}' height='{0}' viewBox='0 0 {0} {0}'>",
            TARGET_SIZE
        ));
        // Background
        // TODO: make that configurable
        svg_out.push_str("<rect width='100%' height='100%' fill='#000000'/>");

        // Draw the segments as lines in the SVG
        for seg in &self.segments {
            let (x1, y1) = map_to_svg(project_point(seg.start), min_x, max_y, scale, PADDING);
            let (x2, y2) = map_to_svg(project_point(seg.end), min_x, max_y, scale, PADDING);

            // Add a shadow line slightly offset from the main line, to help with depth perception.
            svg_out.push_str(&format!(
                "<line x1='{x1:.2}' y1='{y1:.2}' x2='{x2:.2}' y2='{y2:.2}' stroke='{SHADOW_COLOR}' stroke-width='{SHADOW_STROKE_SIZE}' stroke-linecap='round' stroke-opacity='{SHADOW_OPACITY}' transform='translate({SHADOW_OFFSET_X} {SHADOW_OFFSET_Y})'/>"
            ));
            // Main line
            svg_out.push_str(&format!(
                "<line x1='{x1:.2}' y1='{y1:.2}' x2='{x2:.2}' y2='{y2:.2}' stroke='{MODEL_COLOR}' stroke-width='1' stroke-linecap='round'/>"
            ));
        }

        svg_out.push_str("</svg>");

        // Render the SVG to a pixmap using resvg
        let opt = resvg::usvg::Options::default();
        // TODO: use anyhow to handle errors instead of panicking
        let tree =
            resvg::usvg::Tree::from_str(&svg_out, &opt).expect("Generated SVG should be valid");

        let mut pixmap = resvg::tiny_skia::Pixmap::new(TARGET_SIZE, TARGET_SIZE)
            .expect("Failed to create pixmap for rendering");
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::default(),
            &mut pixmap.as_mut(),
        );

        // Convert the rendered pixmap to an image::DynamicImage
        let image =
            image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixmap.data().to_vec())
                .expect("Failed to convert pixmap to image");

        DynamicImage::ImageRgba8(image)
    }
}
impl HasDiagnostics for RenderThumbnailVisitor {
    fn diagnostics(&mut self) -> &mut dyn Diagnostics {
        &mut self.diagnostics
    }
}
impl ProgramVisitor for RenderThumbnailVisitor {
    fn start_block(&mut self) -> ControlFlow<impl BlockVisitor + '_> {
        ControlFlow::Continue(BlockVisitorImpl {
            diagnostics: Noop,
            parent: self,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum CommandKind {
    /// Rapid positioning move.
    G0,
    /// Linear move.
    G1,
    /// Set absolute positioning mode for X/Y/Z coordinates.
    /// Subsequent coordinates are interpreted as absolute positions.
    G90,
    /// Set relative positioning mode for X/Y/Z coordinates.
    /// Subsequent coordinates are interpreted as offsets from the current position.
    G91,
    /// Set the current position without performing movement.
    G92,
    /// Set absolute extrusion mode.
    /// Subsequent E values are interpreted as absolute positions.
    M82,
    /// Set relative extrusion mode.
    /// Subsequent E values are interpreted as offsets from the current extrusion position.
    M83,
    /// Any other command that we choose to ignore.
    Ignore,
}

/// Block visitor implementation.
struct BlockVisitorImpl<'a> {
    diagnostics: Noop,
    parent: &'a mut RenderThumbnailVisitor,
}
impl HasDiagnostics for BlockVisitorImpl<'_> {
    fn diagnostics(&mut self) -> &mut dyn Diagnostics {
        &mut self.diagnostics
    }
}
impl BlockVisitor for BlockVisitorImpl<'_> {
    fn comment(&mut self, value: &str, _span: Span) {
        if !self.parent.ignore_priming_line {
            return;
        }

        // Remove the leading "; "
        let comment_content = value.trim_start_matches(|c: char| c == ';' || c.is_whitespace());

        if comment_content == "LAYER:0" // Cura Slicer
        // Orca Slicer
        || comment_content == "Filament gcode"
        {
            self.parent.segments.clear();
            // We found the priming line marker, we can stop handling comments now
            self.parent.ignore_priming_line = false;
            debug!("Found start of print, cleared prior segments to ignore the priming line");
        }
    }

    fn start_general_code(&mut self, number: Number) -> ControlFlow<impl CommandVisitor + '_> {
        match number.major() {
            // G0: Coordinated Motion at Rapid Rate
            0 => ControlFlow::Continue(CommandVisitorImpl::new(self.parent, CommandKind::G0)),
            // G1: Coordinated Motion at Feed Rate
            1 => ControlFlow::Continue(CommandVisitorImpl::new(self.parent, CommandKind::G1)),
            // G90: Absolute Distance Mode
            90 => ControlFlow::Continue(CommandVisitorImpl::new(self.parent, CommandKind::G90)),
            // G91: Relative Distance Mode
            91 => ControlFlow::Continue(CommandVisitorImpl::new(self.parent, CommandKind::G91)),
            // G92: Coordinate System Offset (used to set the current position)
            92 => ControlFlow::Continue(CommandVisitorImpl::new(self.parent, CommandKind::G92)),

            _ => {
                trace!("Ignoring G{number} instruction");
                ControlFlow::Continue(CommandVisitorImpl::new(self.parent, CommandKind::Ignore))
            }
        }
    }

    fn start_miscellaneous_code(
        &mut self,
        number: Number,
    ) -> ControlFlow<impl CommandVisitor + '_> {
        match number.major() {
            // M82: Set Absolute Extrusion Mode
            82 => ControlFlow::Continue(CommandVisitorImpl::new(self.parent, CommandKind::M82)),
            // M83: Set Relative Extrusion Mode
            83 => ControlFlow::Continue(CommandVisitorImpl::new(self.parent, CommandKind::M83)),

            _ => {
                trace!("Ignoring M{number} instruction");
                ControlFlow::Continue(CommandVisitorImpl::new(self.parent, CommandKind::Ignore))
            }
        }
    }
}

/// Arguments for the current command
#[derive(Debug, Copy, Clone, Default)]
struct CommandArgs {
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
    e: Option<f32>,
}

struct CommandVisitorImpl<'a> {
    diagnostics: Noop,
    parent: &'a mut RenderThumbnailVisitor,
    kind: CommandKind,
    args: CommandArgs,
}
impl HasDiagnostics for CommandVisitorImpl<'_> {
    fn diagnostics(&mut self) -> &mut dyn Diagnostics {
        &mut self.diagnostics
    }
}

impl CommandVisitor for CommandVisitorImpl<'_> {
    fn argument(&mut self, letter: char, value: Value<'_>, _span: Span) {
        // We don't handle non-literal values for arguments
        // TODO: we could maybe handle them?
        let Value::Literal(value) = value else {
            trace!("Non-literal value for argument {letter}, ignoring");
            return;
        };

        // Letters are in uppercase
        match letter {
            'X' => self.args.x = Some(value),
            'Y' => self.args.y = Some(value),
            'Z' => self.args.z = Some(value),
            'E' => self.args.e = Some(value),
            // Ignore other arguments, as we don't use them
            _ => {}
        }
    }

    fn end_command(self, _span: Span) {
        match self.kind {
            CommandKind::Ignore => {}
            g_code @ (CommandKind::G0 | CommandKind::G1) => {
                let old = self.parent.state;

                let extruding = match self.parent.state.extrusion_mode {
                    // In absolute extrusion mode, we're extruding if the E value is strictly greater than the current E position
                    PositionMode::Absolute => self.args.e.is_some_and(|e| e > self.parent.state.e),
                    // In relative extrusion mode, we're extruding if the E value is strictly greater than 0
                    PositionMode::Relative => self.args.e.is_some_and(|e| e > 0.0),
                };

                let state = &mut self.parent.state;
                state.x = apply_axis(state.x, self.args.x, state.distance_mode);
                state.y = apply_axis(state.y, self.args.y, state.distance_mode);
                state.z = apply_axis(state.z, self.args.z, state.distance_mode);
                state.e = apply_axis(state.e, self.args.e, state.extrusion_mode);

                // Only add a segment if we're extruding
                if extruding {
                    self.parent.segments.push(Segment3D {
                        start: Point3D {
                            x: old.x,
                            y: old.y,
                            z: old.z,
                        },
                        end: Point3D {
                            x: state.x,
                            y: state.y,
                            z: state.z,
                        },
                    });
                }

                let delta_e = state.e - old.e;
                trace!(
                    "G{}: X{:.2} Y{:.2} Z{:.2} E{:.2} -> X{:.2} Y{:.2} Z{:.2} E{:.2} dE={:.2}",
                    if g_code == CommandKind::G0 { 0 } else { 1 },
                    old.x,
                    old.y,
                    old.z,
                    old.e,
                    state.x,
                    state.y,
                    state.z,
                    state.e,
                    delta_e,
                );
            }
            CommandKind::G90 => {
                self.parent.state.distance_mode = PositionMode::Absolute;
                trace!("G90: set distance mode absolute");
            }
            CommandKind::G91 => {
                self.parent.state.distance_mode = PositionMode::Relative;
                trace!("G91: set distance mode relative");
            }
            CommandKind::M82 => {
                self.parent.state.extrusion_mode = PositionMode::Absolute;
                trace!("M82: set extrusion mode absolute");
            }
            CommandKind::M83 => {
                self.parent.state.extrusion_mode = PositionMode::Relative;
                trace!("M83: set extrusion mode relative");
            }
            CommandKind::G92 => {
                let state = &mut self.parent.state;
                if let Some(x) = self.args.x {
                    state.x = x;
                }
                if let Some(y) = self.args.y {
                    state.y = y;
                }
                if let Some(z) = self.args.z {
                    state.z = z;
                }
                if let Some(e) = self.args.e {
                    state.e = e;
                }

                trace!(
                    "G92: set position X{:.2} Y{:.2} Z{:.2} E{:.2}",
                    state.x, state.y, state.z, state.e
                );
            }
        }
    }
}

/// Computes a coordinate update according to the active mode.
/// If the value is `None`, the current position is unchanged.
// Having an option as argument isn't the cleanest, but it makes the call site much
// simpler, and it's an internal function anyways.
#[inline] // Very simple function, hint to the compiler that it could be inlined
fn apply_axis(current: f32, value: Option<f32>, mode: PositionMode) -> f32 {
    match (mode, value) {
        (PositionMode::Absolute, Some(value)) => value,
        (PositionMode::Relative, Some(value)) => current + value,
        (_, None) => current,
    }
}

impl<'a> CommandVisitorImpl<'a> {
    fn new(parent: &'a mut RenderThumbnailVisitor, kind: CommandKind) -> Self {
        Self {
            diagnostics: Noop,
            parent,
            kind,
            args: CommandArgs::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RenderThumbnailVisitor;

    #[test]
    /// Checks that in absolute extrusion mode, a segment is only added when
    /// extruding, aka when the E value increases.
    fn test_absolute_extrusion_e_delta() {
        // G90: absolute positioning mode
        // M82: absolute extrusion mode
        const GCODE: &str = r#"
G90
M82
G1 X10 Y0 E1
G1 X20 Y0 E1.5
G1 X30 Y0 E1.1
"#;

        let mut visitor = RenderThumbnailVisitor::new(true);
        gcode::core::parse(GCODE, &mut visitor);

        assert_eq!(visitor.segments.len(), 2);
        assert_eq!(visitor.segments[0].start.x, 0.0);
        assert_eq!(visitor.segments[0].end.x, 10.0);
        assert_eq!(visitor.segments[1].start.x, 10.0);
        assert_eq!(visitor.segments[1].end.x, 20.0);
    }

    #[test]
    /// Checks that in relative extrusion mode, a segment is only added when
    /// extruding, aka when the E value is greater than 0.
    fn test_relative_extrusion_e_delta() {
        // G90: absolute positioning mode
        // M83: relative extrusion mode
        const GCODE: &str = r#"
G90
M83
G1 X5 Y0 E1
G1 X10 Y0 E-1
G1 X15 Y0 E0.5
"#;

        let mut visitor = RenderThumbnailVisitor::new(true);
        gcode::core::parse(GCODE, &mut visitor);

        assert_eq!(visitor.segments.len(), 2);
        assert_eq!(visitor.segments[0].start.x, 0.0);
        assert_eq!(visitor.segments[0].end.x, 5.0);
        assert_eq!(visitor.segments[1].start.x, 10.0);
        assert_eq!(visitor.segments[1].end.x, 15.0);
    }

    #[test]
    /// Checks that the priming line is ignored when the option is enabled, and
    /// isn't ignored when the option is disabled.
    fn test_ignores_priming_line() {
        const GCODE: &str = r#"
G1 X5 Y0 E1
;LAYER:0
G1 X10 Y0 E2
"#;

        // When ignoring the priming lines, only the second segment should be kept
        {
            let mut visitor = RenderThumbnailVisitor::new(true);
            gcode::core::parse(GCODE, &mut visitor);

            assert_eq!(visitor.segments.len(), 1);
            assert_eq!(visitor.segments[0].start.x, 5.0);
            assert_eq!(visitor.segments[0].end.x, 10.0);
        }
        // When not ignoring the priming lines, both segments should be kept
        {
            let mut visitor = RenderThumbnailVisitor::new(false);
            gcode::core::parse(GCODE, &mut visitor);

            assert_eq!(visitor.segments.len(), 2);
            assert_eq!(visitor.segments[0].start.x, 0.0);
            assert_eq!(visitor.segments[0].end.x, 5.0);
            assert_eq!(visitor.segments[1].start.x, 5.0);
            assert_eq!(visitor.segments[1].end.x, 10.0);
        }
    }
}
