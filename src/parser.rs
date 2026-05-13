use gcode::core::{
    BlockVisitor, CommandVisitor, ControlFlow, Diagnostics, HasDiagnostics, Noop, Number,
    ProgramVisitor, Span,
};

/// Program visitor implementation.
/// It does nothing particular, just delegating everything to `BlockVisitorImpl`.
pub struct ProgramVisitorImpl {
    diagnostics: Noop,
}
impl ProgramVisitorImpl {
    pub fn new() -> Self {
        Self { diagnostics: Noop }
    }
}
impl HasDiagnostics for ProgramVisitorImpl {
    fn diagnostics(&mut self) -> &mut dyn Diagnostics {
        &mut self.diagnostics
    }
}
impl ProgramVisitor for ProgramVisitorImpl {
    fn start_block(&mut self) -> ControlFlow<impl BlockVisitor + '_> {
        ControlFlow::Continue(BlockVisitorImpl { diagnostics: Noop })
    }
}

/// Block visitor implementation.
/// It reads comments and G1 instructions.
struct BlockVisitorImpl {
    diagnostics: Noop,
}
impl HasDiagnostics for BlockVisitorImpl {
    fn diagnostics(&mut self) -> &mut dyn Diagnostics {
        &mut self.diagnostics
    }
}
impl BlockVisitor for BlockVisitorImpl {
    fn comment(&mut self, value: &str, _span: Span) {
        println!("Comment: {}", value);
    }

    fn start_general_code(&mut self, number: Number) -> ControlFlow<impl CommandVisitor + '_> {
        if number.major() == 1 {
            println!("G1 instruction");
        }
        ControlFlow::Continue(Noop)
    }

    fn end_line(self, _span: Span) {}
}
