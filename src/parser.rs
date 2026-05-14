use gcode::core::{
    BlockVisitor, CommandVisitor, ControlFlow, Diagnostics, HasDiagnostics, Noop, Number,
    ProgramVisitor, Span,
};
use log::{debug, trace, warn};

/// A thumbnail that was embedded in the G-code file
#[derive(Debug, Clone)]
pub(crate) struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub data: String,
}

/// Program visitor implementation.
/// Tracks thumbnails found in comments and selects the largest one.
pub(crate) struct ProgramVisitorImpl {
    diagnostics: Noop,
    /// The highest quality thumbnail found so far, if any
    best_thumbnail: Option<Thumbnail>,
    /// The thumbnail that is currently being parsed, if any
    current_thumbnail: Option<(u32, u32, String)>, // (width, height, data)
}
impl ProgramVisitorImpl {
    pub fn new() -> Self {
        Self {
            diagnostics: Noop,
            best_thumbnail: None,
            current_thumbnail: None,
        }
    }

    /// Get the largest thumbnail found, if any.
    pub fn get_best_thumbnail(&self) -> Option<&Thumbnail> {
        self.best_thumbnail.as_ref()
    }
}
impl HasDiagnostics for ProgramVisitorImpl {
    fn diagnostics(&mut self) -> &mut dyn Diagnostics {
        &mut self.diagnostics
    }
}
impl ProgramVisitor for ProgramVisitorImpl {
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
    parent: &'a mut ProgramVisitorImpl,
}
impl HasDiagnostics for BlockVisitorImpl<'_> {
    fn diagnostics(&mut self) -> &mut dyn Diagnostics {
        &mut self.diagnostics
    }
}
impl BlockVisitor for BlockVisitorImpl<'_> {
    fn comment(&mut self, value: &str, _span: Span) {
        trace!("Comment: {value}");

        // Remove the leading "; "
        let comment_content = value.trim_start_matches(|c: char| c == ';' || c.is_whitespace());

        // Check for the thumbnail begin section
        if let Some(thumb_meta) = comment_content.strip_prefix("thumbnail begin ")
            && let Some((dims, size_str)) = thumb_meta.split_once(' ')
            && let Some((width_str, height_str)) = dims.split_once('x')
            && let (Ok(width), Ok(height), Ok(_size)) = (
                width_str.parse::<u32>(),
                height_str.parse::<u32>(),
                size_str.parse::<u32>(),
            )
        {
            self.parent.current_thumbnail = Some((width, height, String::new()));
            debug!("Found thumbnail block: {width}x{height} (size {size_str})");
        }
        // Check for the thumbnail end section
        else if comment_content == "thumbnail end" {
            if let Some((width, height, data)) = self.parent.current_thumbnail.take() {
                let thumbnail = Thumbnail {
                    width,
                    height,
                    data,
                };
                debug!(
                    "Completed thumbnail block: {width}x{height} ({} bytes)",
                    thumbnail.data.len()
                );
                // Update the best thumbnail if this one is larger
                if self.parent.best_thumbnail.as_ref().is_none_or(|best| {
                    // Cast to u64 to avoid overflow when multiplying, unlikely but you never know
                    (thumbnail.width as u64 * thumbnail.height as u64)
                        > (best.width as u64 * best.height as u64)
                }) {
                    self.parent.best_thumbnail = Some(thumbnail);
                    debug!("Updated best thumbnail to {width}x{height}");
                }
            } else {
                warn!("Found `thumbnail end` without a prior `thumbnail begin`");
            }
        }
        // Accumulate thumbnail data
        else if let Some((_, _, ref mut data)) = self.parent.current_thumbnail {
            data.push_str(comment_content);
        }
    }

    fn start_general_code(&mut self, number: Number) -> ControlFlow<impl CommandVisitor + '_> {
        if number.major() == 1 {
            debug!("Found G1 instruction, exiting parsing");
            ControlFlow::<Noop>::Break(())
        } else {
            trace!("Found G{} instruction, ignoring it", number);
            ControlFlow::Continue(Noop)
        }
    }

    fn end_line(self, _span: Span) {}
}
