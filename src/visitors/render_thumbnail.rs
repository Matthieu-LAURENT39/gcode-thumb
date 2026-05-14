use gcode::core::{
    BlockVisitor, CommandVisitor, ControlFlow, Diagnostics, HasDiagnostics, Noop, Number,
    ProgramVisitor,
};
use log::{debug, trace};

/// Parses the G-code file and renders a thumbnail from it.
pub(crate) struct RenderThumbnailVisitor {
    diagnostics: Noop,
}
impl RenderThumbnailVisitor {
    pub fn new() -> Self {
        Self { diagnostics: Noop }
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
        if number.major() == 1 {
            debug!("Found G1 instruction, exiting parsing");
            ControlFlow::<Noop>::Break(())
        } else {
            trace!("Found G{} instruction, ignoring it", number);
            ControlFlow::Continue(Noop)
        }
    }
}
