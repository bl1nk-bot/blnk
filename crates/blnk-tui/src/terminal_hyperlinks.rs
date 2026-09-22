//! Terminal hyperlink (OSC 8) support for ratatui lines.

use ratatui::text::Line;
use std::ops::Range;

/// A clickable hyperlink attached to a column range within a line.
#[derive(Clone, Debug)]
pub struct Hyperlink {
    pub url: String,
    pub columns: Range<usize>,
}

/// A ratatui `Line` annotated with hyperlink regions.
#[derive(Clone, Debug)]
pub struct HyperlinkLine {
    pub line: Line<'static>,
    pub hyperlinks: Vec<Hyperlink>,
}

impl HyperlinkLine {
    pub fn new(line: Line<'static>) -> Self {
        Self {
            line,
            hyperlinks: Vec::new(),
        }
    }

    pub fn width(&self) -> usize {
        self.line.width()
    }
}

/// Remap hyperlink column offsets after wrapping a source line into multiple output lines.
pub fn remap_wrapped_line(
    source: &HyperlinkLine,
    wrapped: Vec<Line<'static>>,
) -> Vec<HyperlinkLine> {
    let mut result = Vec::new();
    let mut col_offset = 0usize;

    for line in wrapped {
        let line_width = line.width();
        let mut hyperlinks = Vec::new();

        for link in &source.hyperlinks {
            let link_start = link.columns.start;
            let link_end = link.columns.end;

            // Check if this wrapped line overlaps with the hyperlink
            let vis_start = link_start.saturating_sub(col_offset);
            let vis_end = link_end.saturating_sub(col_offset);

            if vis_end > 0 && vis_start < line_width {
                hyperlinks.push(Hyperlink {
                    url: link.url.clone(),
                    columns: vis_start..vis_end.min(line_width),
                });
            }
        }

        result.push(HyperlinkLine { line, hyperlinks });
        col_offset += line_width;
    }

    result
}
