//! Streaming markdown render metadata collected during the writer's single parse pass.
//!
//! Top-level block offsets always refer to the exact source passed to this renderer; callers that
//! normalize source before rendering must not apply those offsets to the original source.
#![allow(dead_code)]

use pulldown_cmark::{Event, Tag};
use std::ops::Range;

use crate::terminal_hyperlinks::HyperlinkLine;

/// Rendered lines and the block metadata needed to keep only the final block mutable.
pub(crate) struct StreamingMarkdownRender {
    /// Styled output produced by the same parser pass that collected the metadata below.
    pub(crate) lines: Vec<HyperlinkLine>,
    /// Source line containing an unfinished display equation, which must not enter scrollback.
    pub(crate) pending_math_start: Option<usize>,
    /// Byte offset of the final top-level block when at least one earlier block exists.
    pub(crate) last_top_level_block_start: Option<usize>,
    /// Whether a reference definition can retroactively change another block's rendering.
    pub(crate) has_reference_link_definition: bool,
    /// Whether the first block is raw HTML, which joins a retained prefix without a separator.
    pub(crate) first_top_level_block_is_html: bool,
}

/// Records top-level block boundaries without adding a second parser traversal.
struct TopLevelBlockTracker<'a, I> {
    iter: I,
    depth: usize,
    block_count: usize,
    last_start: usize,
    first_is_html: bool,
    _marker: std::marker::PhantomData<&'a str>,
}

impl<'a, I> Iterator for TopLevelBlockTracker<'a, I>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    type Item = (Event<'a>, Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        let (event, range) = self.iter.next()?;
        if self.depth == 0 && matches!(&event, Event::Start(_) | Event::Rule | Event::Html(_)) {
            self.block_count += 1;
            self.last_start = range.start;
            if self.block_count == 1 {
                self.first_is_html =
                    matches!(&event, Event::Start(Tag::HtmlBlock) | Event::Html(_));
            }
        }
        match event {
            Event::Start(_) => self.depth += 1,
            Event::End(_) => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
        Some((event, range))
    }
}

// TODO: implement render_streaming_markdown_lines_with_width_and_cwd
// Requires: DecodedTextMerge, FileCitations, MathMarkdown, Writer, Options setup
