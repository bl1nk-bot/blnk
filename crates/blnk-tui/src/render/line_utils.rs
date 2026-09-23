use ratatui::text::Line;

pub fn line_to_static(line: &Line<'_>) -> Line<'static> {
    Line {
        style: line.style,
        alignment: line.alignment,
        spans: line
            .spans
            .iter()
            .map(|span| ratatui::text::Span {
                style: span.style,
                content: std::borrow::Cow::Owned(span.content.to_string()),
            })
            .collect(),
    }
}

pub fn push_owned_lines(source: &[Line<'_>], target: &mut Vec<Line<'static>>) {
    for line in source {
        target.push(line_to_static(line));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Style};
    use ratatui::text::Span;

    #[test]
    fn line_to_static_preserves_style_and_alignment() {
        let line = Line {
            style: Style::default().fg(Color::Yellow),
            alignment: Some(ratatui::layout::Alignment::Center),
            spans: vec![Span::styled("hello", Style::default().bg(Color::Blue))],
        };

        let static_line = line_to_static(&line);
        assert_eq!(static_line.style, line.style);
        assert_eq!(static_line.alignment, line.alignment);
        assert_eq!(static_line.spans.len(), 1);
        assert_eq!(static_line.spans[0].content, "hello");
        assert_eq!(static_line.spans[0].style, line.spans[0].style);
    }
}
