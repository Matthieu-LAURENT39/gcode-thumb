use gcode::core::{
    BlockVisitor, CommandVisitor, ControlFlow, Diagnostics, HasDiagnostics, Noop, Number,
    ProgramVisitor, Span, Value,
};
use image::DynamicImage;
use log::trace;

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

/// Parses the G-code file and renders a thumbnail from it.
pub(crate) struct RenderThumbnailVisitor {
    diagnostics: Noop,
    state: PrinterState,
}
impl RenderThumbnailVisitor {
    pub fn new() -> Self {
        Self {
            diagnostics: Noop,
            state: PrinterState::default(),
        }
    }

    pub fn render(&self) -> DynamicImage {
        todo!("Add G-code rendering")
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
                let state = &mut self.parent.state;

                let old = *state;

                state.x = apply_axis(state.x, self.args.x, state.distance_mode);
                state.y = apply_axis(state.y, self.args.y, state.distance_mode);
                state.z = apply_axis(state.z, self.args.z, state.distance_mode);
                state.e = apply_axis(state.e, self.args.e, state.extrusion_mode);

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

    // Determines if the current command is an extruding move based on the E argument
    fn is_extruding(&self) -> bool {
        match self.parent.state.extrusion_mode {
            // In absolute extrusion mode, we're extruding if the E value is strictly greater than the current E position
            PositionMode::Absolute => self.args.e.is_some_and(|e| e > self.parent.state.e),
            // In relative extrusion mode, we're extruding if the E value is strictly greater than 0
            PositionMode::Relative => self.args.e.is_some_and(|e| e > 0.0),
        }
    }
}
