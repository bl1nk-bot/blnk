use ratatui::text::Line;

pub fn line_to_static(line: &Line<'_>) -> Line<'static> {
    Line::from(
        line.spans
            .iter()
            .map(|span| ratatui::text::Span::styled(span.content.to_string(), span.style))
            .collect::<Vec<_>>(),
    )
}

pub fn push_owned_lines(source: &[Line<'_>], target: &mut Vec<Line<'static>>) {
    for line in source {
        target.push(line_to_static(line));
    }
}
