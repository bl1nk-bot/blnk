//! Terminal hyperlink (OSC 8) support for ratatui lines.

use ratatui::text::Line;
use std::ops::Range;

fn source_columns_for_output_line(
    source: &Line<'_>,
    wrapped: &Line<'_>,
    consumed: usize,
) -> (usize, usize) {
    let source_text = source
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    let output_text = wrapped
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();

    let remaining = source_text.get(consumed..).unwrap_or_default();
    let trimmed = output_text.trim();
    if trimmed.is_empty() {
        return (consumed, consumed);
    }

    let relative_start = remaining.find(trimmed).unwrap_or(0);
    let mapped_start = consumed + relative_start;
    let mapped_end = mapped_start + trimmed.len();
    (mapped_start, mapped_end)
}

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
        Self { line, hyperlinks: Vec::new() }
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
    let mut source_offset = 0usize;

    for line in wrapped {
        let line_width = line.width();
        let mut hyperlinks = Vec::new();
        let (mapped_start, mapped_end) =
            source_columns_for_output_line(&source.line, &line, source_offset);

        for link in &source.hyperlinks {
            let link_start = link.columns.start;
            let link_end = link.columns.end;

            if link_end <= mapped_start {
                continue;
            }
            if link_start >= mapped_end {
                continue;
            }

            // Check if this wrapped line overlaps with the hyperlink
            let vis_start = link_start.saturating_sub(mapped_start).max(0);
            let vis_end = link_end.saturating_sub(mapped_start).min(line_width);

            if vis_end > 0 && vis_start < line_width {
                hyperlinks.push(Hyperlink {
                    url: link.url.clone(),
                    columns: vis_start..vis_end.min(line_width),
                });
            }
        }

        let next_offset = source_columns_for_output_line(&source.line, &line, source_offset).1;
        result.push(HyperlinkLine { line, hyperlinks });
        source_offset = next_offset;
        if source_offset < source.line.width()
            && source
                .line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
                .as_bytes()[source_offset]
                == b' '
        {
            source_offset += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::{Line, Span};

    #[test]
    fn remap_wrapped_line_keeps_hyperlinks_after_first_wrap() {
        let line = Line::from(vec![Span::raw("prefix https://example.com")]);
        let source = HyperlinkLine {
            line,
            hyperlinks: vec![Hyperlink {
                url: "https://example.com".to_owned(),
                columns: 7..26,
            }],
        };

        let wrapped = vec![
            Line::from(vec![Span::raw("prefix")]),
            Line::from(vec![Span::raw("https://example.com")]),
        ];

        let remapped = remap_wrapped_line(&source, wrapped);
        assert_eq!(remapped.len(), 2);
        assert!(
            remapped[1]
                .hyperlinks
                .iter()
                .any(|link| link.columns == (0..19))
        );
    }
}
