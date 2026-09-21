//! Word-wrapping with URL-aware heuristics.
//!
//! The TUI renders text that frequently contains URLs — command output,
//! markdown, agent messages, tool-call results. Standard `textwrap`
//! hyphenation treats `/` and `-` as split points, which breaks URLs
//! across lines and makes them unclickable in terminal emulators.
//!
//! This module provides two wrapping paths:
//!
//! - **Standard** (`word_wrap_line`, `word_wrap_lines`): delegates to
//!   `textwrap` with the caller's options unchanged. Used when the
//!   content is known to be plain prose.
//! - **Adaptive** (`adaptive_wrap_line`, `adaptive_wrap_lines`):
//!   inspects the line for URL-like tokens; if any are found, the
//!   wrapping keeps URL tokens intact. Mixed URL/prose lines still wrap
//!   ordinary prose at word boundaries, only splitting a non-URL token
//!   when that token is itself wider than the available row width.
//!
//! Callers that *might* encounter URLs should use the `adaptive_*`
//! functions. Callers that definitely will not (code blocks, pure
//! numeric output) can use the standard path for speed.
//!
//! URL detection is heuristic — see [`text_contains_url_like`] for the
//! rules. False positives suppress hyphenation for that line; false
//! negatives let a URL get split. The heuristic is intentionally
//! conservative: file paths like `src/main.rs` are not matched.
#![allow(dead_code)]

use ratatui::text::Line;
use ratatui::text::Span;
use std::borrow::Cow;
use std::ops::Range;
use textwrap::Options;
use textwrap::WordSeparator;
use textwrap::core::Word;
use textwrap::word_splitters::split_words;
use unicode_segmentation::UnicodeSegmentation;

use crate::render::line_utils::push_owned_lines;
use crate::width::display_width;

/// Compute display width of a ratatui Line by summing span text widths.
fn line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| display_width(span.content.as_ref()))
        .sum()
}

// ── Halfwidth sound mark projection ────────────────────────────────────

struct ProjectedText {
    text: String,
    source_boundaries: Vec<(usize, usize)>,
    grapheme_boundaries: Vec<usize>,
}

fn project_halfwidth_sound_marks(text: &str) -> Option<ProjectedText> {
    if !text.contains(['\u{FF9E}', '\u{FF9F}']) {
        return None;
    }

    let mut projected = String::with_capacity(text.len());
    let mut source_boundaries = vec![(0, 0)];
    let mut grapheme_boundaries = vec![0];
    for (source_start, grapheme) in text.grapheme_indices(true) {
        if grapheme.contains(['\u{FF9E}', '\u{FF9F}']) {
            let source_end = source_start + grapheme.len();
            let content_start = grapheme
                .find(|ch: char| !ch.is_whitespace())
                .unwrap_or(grapheme.len());
            let (whitespace, content) = grapheme.split_at(content_start);
            for (offset, ch) in whitespace.char_indices() {
                projected.push(ch);
                source_boundaries.push((projected.len(), source_start + offset + ch.len_utf8()));
                grapheme_boundaries.push(projected.len());
            }

            let width = display_width(content);
            let projected_start = projected.len();
            for _ in 0..width / 2 {
                if projected.len() > projected_start {
                    projected.push('\u{2060}');
                }
                projected.push('界');
                source_boundaries.push((projected.len(), source_end));
            }
            if width % 2 == 1 {
                if projected.len() > projected_start {
                    projected.push('\u{2060}');
                }
                projected.push('a');
                source_boundaries.push((projected.len(), source_end));
            }
        } else {
            for (offset, ch) in grapheme.char_indices() {
                projected.push(ch);
                source_boundaries.push((projected.len(), source_start + offset + ch.len_utf8()));
            }
        }
        grapheme_boundaries.push(projected.len());
    }

    Some(ProjectedText {
        text: projected,
        source_boundaries,
        grapheme_boundaries,
    })
}

fn source_offset(boundaries: &[(usize, usize)], projected_offset: usize) -> usize {
    boundaries
        .binary_search_by_key(&projected_offset, |(offset, _)| *offset)
        .map(|index| boundaries[index].1)
        .unwrap_or(projected_offset)
}

fn break_projected_words<'a>(
    words: impl Iterator<Item = Word<'a>>,
    projected: &'a ProjectedText,
    line_width: usize,
) -> Vec<Word<'a>> {
    let projected_start = projected.text.as_ptr() as usize;
    let mut pieces = Vec::new();

    for word in words {
        if display_width(word.word) <= line_width {
            pieces.push(word);
            continue;
        }

        let word_start = word.word.as_ptr() as usize - projected_start;
        let word_end = word_start + word.word.len();
        let mut piece_start = word_start;
        let mut piece_width = 0;
        let mut atom_start = word_start;
        let boundary_start = projected
            .grapheme_boundaries
            .partition_point(|atom_end| *atom_end <= word_start);

        for atom_end in projected
            .grapheme_boundaries
            .iter()
            .copied()
            .skip(boundary_start)
        {
            if atom_end > word_end {
                break;
            }

            let atom_width = display_width(&projected.text[atom_start..atom_end]);
            if piece_width > 0 && piece_width + atom_width > line_width {
                pieces.push(Word::from(&projected.text[piece_start..atom_start]));
                piece_start = atom_start;
                piece_width = 0;
            }
            piece_width += atom_width;
            atom_start = atom_end;
        }

        let mut last = Word::from(&projected.text[piece_start..word_end]);
        last.whitespace = word.whitespace;
        last.penalty = word.penalty;
        pieces.push(last);
    }

    pieces
}

fn wrap_projected_ranges(
    projected: &ProjectedText,
    opts: &Options<'_>,
    include_trailing_spaces: bool,
) -> Vec<Range<usize>> {
    let line_widths = [
        opts.width
            .saturating_sub(display_width(opts.initial_indent)),
        opts.width
            .saturating_sub(display_width(opts.subsequent_indent)),
    ];
    let line_ending = opts.line_ending.as_str();
    let mut ranges = Vec::new();
    let mut line_start = 0;

    for line in projected.text.split(line_ending) {
        let words = opts.word_separator.find_words(line);
        let split_words = split_words(words, &opts.word_splitter);
        let mut broken_words = if opts.break_words {
            break_projected_words(split_words, projected, line_widths[1])
        } else {
            split_words.collect()
        };
        if opts.break_words && !opts.initial_indent.is_empty() {
            broken_words.insert(0, Word::from(""));
        }

        let wrapped_words = opts.wrap_algorithm.wrap(&broken_words, &line_widths);
        let mut cursor = line_start;
        for words in wrapped_words {
            let Some(last_word) = words.last() else {
                let source = source_offset(&projected.source_boundaries, cursor);
                ranges.push(source..source + usize::from(include_trailing_spaces));
                continue;
            };
            let len = words
                .iter()
                .map(|word| word.word.len() + word.whitespace.len())
                .sum::<usize>()
                - last_word.whitespace.len();
            let end = cursor + len;
            let trailing_spaces = if include_trailing_spaces {
                projected.text[end..]
                    .chars()
                    .take_while(|ch| *ch == ' ')
                    .count()
            } else {
                0
            };
            let source_start = source_offset(&projected.source_boundaries, cursor);
            let source_end = source_offset(&projected.source_boundaries, end + trailing_spaces);
            ranges.push(source_start..source_end + usize::from(include_trailing_spaces));
            cursor = end + last_word.whitespace.len();
        }
        line_start += line.len() + line_ending.len();
    }

    ranges
}

// ── Range utilities ────────────────────────────────────────────────────

pub(crate) fn wrap_ranges<'a, O>(text: &str, width_or_options: O) -> Vec<Range<usize>>
where
    O: Into<Options<'a>>,
{
    let opts = width_or_options.into();
    if let Some(projected) = project_halfwidth_sound_marks(text) {
        return wrap_projected_ranges(&projected, &opts, true);
    }
    let mut lines: Vec<Range<usize>> = Vec::new();
    let mut cursor = 0usize;
    for (line_index, line) in textwrap::wrap(text, &opts).iter().enumerate() {
        match line {
            Cow::Borrowed(slice) => {
                let range = borrowed_slice_range(text, slice).unwrap_or_else(|| {
                    let synthetic_prefix = if line_index == 0 {
                        opts.initial_indent
                    } else {
                        opts.subsequent_indent
                    };
                    map_owned_wrapped_line_to_range(text, cursor, slice, synthetic_prefix)
                });
                let start = range.start;
                let end = range.end;
                let trailing_spaces = text[end..].chars().take_while(|c| *c == ' ').count();
                lines.push(start..end + trailing_spaces + 1);
                cursor = end + trailing_spaces;
            }
            Cow::Owned(slice) => {
                let synthetic_prefix = if line_index == 0 {
                    opts.initial_indent
                } else {
                    opts.subsequent_indent
                };
                let mapped = map_owned_wrapped_line_to_range(text, cursor, slice, synthetic_prefix);
                let trailing_spaces = text[mapped.end..].chars().take_while(|c| *c == ' ').count();
                lines.push(mapped.start..mapped.end + trailing_spaces + 1);
                cursor = mapped.end + trailing_spaces;
            }
        }
    }
    lines
}

pub(crate) fn wrap_ranges_trim<'a, O>(text: &str, width_or_options: O) -> Vec<Range<usize>>
where
    O: Into<Options<'a>>,
{
    let opts = width_or_options.into();
    if let Some(projected) = project_halfwidth_sound_marks(text) {
        return wrap_projected_ranges(&projected, &opts, false);
    }
    let mut lines: Vec<Range<usize>> = Vec::new();
    let mut cursor = 0usize;
    for (line_index, line) in textwrap::wrap(text, &opts).iter().enumerate() {
        match line {
            Cow::Borrowed(slice) => {
                let range = borrowed_slice_range(text, slice).unwrap_or_else(|| {
                    let synthetic_prefix = if line_index == 0 {
                        opts.initial_indent
                    } else {
                        opts.subsequent_indent
                    };
                    map_owned_wrapped_line_to_range(text, cursor, slice, synthetic_prefix)
                });
                cursor = range.end;
                lines.push(range);
            }
            Cow::Owned(slice) => {
                let synthetic_prefix = if line_index == 0 {
                    opts.initial_indent
                } else {
                    opts.subsequent_indent
                };
                let mapped = map_owned_wrapped_line_to_range(text, cursor, slice, synthetic_prefix);
                lines.push(mapped.clone());
                cursor = mapped.end;
            }
        }
    }
    lines
}

fn borrowed_slice_range(text: &str, slice: &str) -> Option<Range<usize>> {
    let text_start = text.as_ptr() as usize;
    let text_end = text_start.checked_add(text.len())?;
    let slice_start = slice.as_ptr() as usize;
    let slice_end = slice_start.checked_add(slice.len())?;

    if slice_start < text_start || slice_end > text_end {
        return None;
    }

    Some((slice_start - text_start)..(slice_end - text_start))
}

fn map_owned_wrapped_line_to_range(
    text: &str,
    cursor: usize,
    wrapped: &str,
    synthetic_prefix: &str,
) -> Range<usize> {
    let wrapped = if synthetic_prefix.is_empty() {
        wrapped
    } else {
        wrapped.strip_prefix(synthetic_prefix).unwrap_or(wrapped)
    };

    let mut start = cursor;
    while start < text.len() && !wrapped.starts_with(' ') {
        let Some(ch) = text[start..].chars().next() else {
            break;
        };
        if ch != ' ' {
            break;
        }
        start += ch.len_utf8();
    }

    let mut end = start;
    let mut saw_source_char = false;
    let mut chars = wrapped.chars().peekable();
    while let Some(ch) = chars.next() {
        if end < text.len() {
            let Some(src) = text[end..].chars().next() else {
                unreachable!("checked end < text.len()");
            };
            if ch == src {
                end += src.len_utf8();
                saw_source_char = true;
                continue;
            }
        }

        if ch == '-' && chars.peek().is_none() {
            continue;
        }

        if !saw_source_char {
            continue;
        }

        tracing::warn!(
            wrapped = %wrapped,
            cursor,
            end,
            "wrap_ranges: could not fully map owned line; returning partial source range"
        );
        break;
    }

    start..end
}

// ── URL detection ──────────────────────────────────────────────────────

pub(crate) fn line_contains_url_like(line: &Line<'_>) -> bool {
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    text_contains_url_like(&text)
}

pub(crate) fn line_has_mixed_url_and_non_url_tokens(line: &Line<'_>) -> bool {
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    text_has_mixed_url_and_non_url_tokens(&text)
}

pub(crate) fn text_contains_url_like(text: &str) -> bool {
    text.split_ascii_whitespace().any(is_url_like_token)
}

fn text_has_mixed_url_and_non_url_tokens(text: &str) -> bool {
    let mut saw_url = false;
    let mut saw_non_url = false;

    for raw_token in text.split_ascii_whitespace() {
        if is_url_like_token(raw_token) {
            saw_url = true;
        } else if is_substantive_non_url_token(raw_token) {
            saw_non_url = true;
        }

        if saw_url && saw_non_url {
            return true;
        }
    }

    false
}

fn is_url_like_token(raw_token: &str) -> bool {
    let token = trim_url_token(raw_token);
    !token.is_empty() && (is_absolute_url_like(token) || is_bare_url_like(token))
}

fn is_substantive_non_url_token(raw_token: &str) -> bool {
    let token = trim_url_token(raw_token);
    if token.is_empty() || is_decorative_marker_token(raw_token, token) {
        return false;
    }

    token.chars().any(char::is_alphanumeric)
}

fn is_decorative_marker_token(raw_token: &str, token: &str) -> bool {
    let raw = raw_token.trim();
    matches!(
        raw,
        "-" | "*"
            | "+"
            | "•"
            | "◦"
            | "▪"
            | ">"
            | "|"
            | "│"
            | "┆"
            | "└"
            | "├"
            | "┌"
            | "┐"
            | "┘"
            | "┼"
    ) || is_ordered_list_marker(raw, token)
}

fn is_ordered_list_marker(raw_token: &str, token: &str) -> bool {
    token.chars().all(|c| c.is_ascii_digit())
        && (raw_token.ends_with('.') || raw_token.ends_with(')'))
}

fn trim_url_token(token: &str) -> &str {
    token.trim_matches(|c: char| {
        matches!(
            c,
            '(' | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | ','
                | '.'
                | ';'
                | ':'
                | '!'
                | '\''
                | '"'
        )
    })
}

fn is_absolute_url_like(token: &str) -> bool {
    if !token.contains("://") {
        return false;
    }

    if let Ok(url) = url::Url::parse(token) {
        let scheme = url.scheme().to_ascii_lowercase();
        if matches!(
            scheme.as_str(),
            "http" | "https" | "ftp" | "ftps" | "ws" | "wss"
        ) {
            return url.host_str().is_some();
        }
        return true;
    }

    has_valid_scheme_prefix(token)
}

fn has_valid_scheme_prefix(token: &str) -> bool {
    let Some((scheme, rest)) = token.split_once("://") else {
        return false;
    };
    if scheme.is_empty() || rest.is_empty() {
        return false;
    }

    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
}

fn is_bare_url_like(token: &str) -> bool {
    let (host_port, has_trailer) = split_host_port_and_trailer(token);
    if host_port.is_empty() {
        return false;
    }

    if !has_trailer && !host_port.to_ascii_lowercase().starts_with("www.") {
        return false;
    }

    let (host, port) = split_host_and_port(host_port);
    if host.is_empty() {
        return false;
    }
    if let Some(port) = port
        && !is_valid_port(port)
    {
        return false;
    }

    host.eq_ignore_ascii_case("localhost") || is_ipv4(host) || is_domain_name(host)
}

fn split_host_port_and_trailer(token: &str) -> (&str, bool) {
    if let Some(idx) = token.find(['/', '?', '#']) {
        (&token[..idx], true)
    } else {
        (token, false)
    }
}

fn split_host_and_port(host_port: &str) -> (&str, Option<&str>) {
    if host_port.starts_with('[') {
        return (host_port, None);
    }

    if let Some((host, port)) = host_port.rsplit_once(':')
        && !host.is_empty()
        && !port.is_empty()
        && port.chars().all(|c| c.is_ascii_digit())
    {
        return (host, Some(port));
    }

    (host_port, None)
}

fn is_valid_port(port: &str) -> bool {
    if port.is_empty() || port.len() > 5 || !port.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    port.parse::<u16>().is_ok()
}

fn is_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() != 4 {
        return false;
    }

    parts
        .iter()
        .all(|part| !part.is_empty() && part.parse::<u8>().is_ok())
}

fn is_domain_name(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if !host.contains('.') {
        return false;
    }

    let mut labels = host.split('.');
    let Some(tld) = labels.next_back() else {
        return false;
    };
    if !is_tld(tld) {
        return false;
    }

    labels.all(is_domain_label)
}

fn is_tld(label: &str) -> bool {
    (2..=63).contains(&label.len()) && label.chars().all(|c| c.is_ascii_alphabetic())
}

fn is_domain_label(label: &str) -> bool {
    if label.is_empty() || label.len() > 63 {
        return false;
    }

    let mut chars = label.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let Some(last) = label.chars().next_back() else {
        return false;
    };

    first.is_ascii_alphanumeric()
        && last.is_ascii_alphanumeric()
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

// ── RtOptions ──────────────────────────────────────────────────────────

pub(crate) fn url_preserving_wrap_options<'a>(opts: RtOptions<'a>) -> RtOptions<'a> {
    opts.word_separator(textwrap::WordSeparator::AsciiSpace)
        .word_splitter(textwrap::WordSplitter::NoHyphenation)
        .break_words(false)
}

#[derive(Clone, Debug)]
pub struct RtOptions<'a> {
    pub width: usize,
    pub line_ending: textwrap::LineEnding,
    pub initial_indent: Line<'a>,
    pub subsequent_indent: Line<'a>,
    pub break_words: bool,
    pub wrap_algorithm: textwrap::WrapAlgorithm,
    pub word_separator: textwrap::WordSeparator,
    pub word_splitter: textwrap::WordSplitter,
}

impl From<usize> for RtOptions<'_> {
    fn from(width: usize) -> Self {
        RtOptions::new(width)
    }
}

impl<'a> RtOptions<'a> {
    pub fn new(width: usize) -> Self {
        RtOptions {
            width,
            line_ending: textwrap::LineEnding::LF,
            initial_indent: Line::default(),
            subsequent_indent: Line::default(),
            break_words: true,
            word_separator: textwrap::WordSeparator::new(),
            wrap_algorithm: textwrap::WrapAlgorithm::FirstFit,
            word_splitter: textwrap::WordSplitter::HyphenSplitter,
        }
    }

    pub fn initial_indent(self, initial_indent: Line<'a>) -> Self {
        RtOptions {
            initial_indent,
            ..self
        }
    }

    pub fn subsequent_indent(self, subsequent_indent: Line<'a>) -> Self {
        RtOptions {
            subsequent_indent,
            ..self
        }
    }

    pub fn break_words(self, break_words: bool) -> Self {
        RtOptions {
            break_words,
            ..self
        }
    }

    pub fn word_separator(self, word_separator: textwrap::WordSeparator) -> RtOptions<'a> {
        RtOptions {
            word_separator,
            ..self
        }
    }

    pub fn wrap_algorithm(self, wrap_algorithm: textwrap::WrapAlgorithm) -> RtOptions<'a> {
        RtOptions {
            wrap_algorithm,
            ..self
        }
    }

    pub fn word_splitter(self, word_splitter: textwrap::WordSplitter) -> RtOptions<'a> {
        RtOptions {
            word_splitter,
            ..self
        }
    }
}

// ── Standard wrapping ──────────────────────────────────────────────────

pub(crate) fn word_wrap_line<'a, O>(line: &'a Line<'a>, width_or_options: O) -> Vec<Line<'a>>
where
    O: Into<RtOptions<'a>>,
{
    let (flat, span_bounds) = flatten_line(line);
    word_wrap_flattened_line(line, &flat, &span_bounds, width_or_options.into())
}

fn word_wrap_flattened_line<'a>(
    line: &'a Line<'a>,
    flat: &str,
    span_bounds: &[(Range<usize>, ratatui::style::Style)],
    rt_opts: RtOptions<'a>,
) -> Vec<Line<'a>> {
    let opts = Options::new(rt_opts.width)
        .line_ending(rt_opts.line_ending)
        .break_words(rt_opts.break_words)
        .wrap_algorithm(rt_opts.wrap_algorithm)
        .word_separator(rt_opts.word_separator)
        .word_splitter(rt_opts.word_splitter);

    let mut out: Vec<Line<'a>> = Vec::new();

    let initial_width_available = opts
        .width
        .saturating_sub(line_width(&rt_opts.initial_indent))
        .max(1);
    let initial_wrapped = wrap_ranges_trim(flat, opts.clone().width(initial_width_available));
    let Some(first_line_range) = initial_wrapped.first() else {
        return vec![rt_opts.initial_indent.clone()];
    };

    let mut first_line = rt_opts.initial_indent.clone().style(line.style);
    {
        let sliced = slice_line_spans(line, span_bounds, first_line_range);
        let mut spans = first_line.spans;
        spans.append(
            &mut sliced
                .spans
                .into_iter()
                .map(|s| s.patch_style(line.style))
                .collect(),
        );
        first_line.spans = spans;
        out.push(first_line);
    }

    let base = first_line_range.end;
    let skip_leading_spaces = flat[base..].chars().take_while(|c| *c == ' ').count();
    let base = base + skip_leading_spaces;
    let subsequent_width_available = opts
        .width
        .saturating_sub(line_width(&rt_opts.subsequent_indent))
        .max(1);
    let remaining_wrapped = wrap_ranges_trim(&flat[base..], opts.width(subsequent_width_available));
    for r in &remaining_wrapped {
        if r.is_empty() {
            continue;
        }
        let mut subsequent_line = rt_opts.subsequent_indent.clone().style(line.style);
        let offset_range = (r.start + base)..(r.end + base);
        let sliced = slice_line_spans(line, span_bounds, &offset_range);
        let mut spans = subsequent_line.spans;
        spans.append(
            &mut sliced
                .spans
                .into_iter()
                .map(|s| s.patch_style(line.style))
                .collect(),
        );
        subsequent_line.spans = spans;
        out.push(subsequent_line);
    }

    out
}

pub(crate) fn word_wrap_lines<'a, I, O, L>(lines: I, width_or_options: O) -> Vec<Line<'static>>
where
    I: IntoIterator<Item = L>,
    L: IntoLineInput<'a>,
    O: Into<RtOptions<'a>>,
{
    let base_opts: RtOptions<'a> = width_or_options.into();
    let mut out: Vec<Line<'static>> = Vec::new();

    for (idx, line) in lines.into_iter().enumerate() {
        let line_input = line.into_line_input();
        let opts = if idx == 0 {
            base_opts.clone()
        } else {
            let mut o = base_opts.clone();
            let sub = o.subsequent_indent.clone();
            o = o.initial_indent(sub);
            o
        };
        let wrapped = word_wrap_line(line_input.as_ref(), opts);
        push_owned_lines(&wrapped, &mut out);
    }

    out
}

// ── Adaptive (URL-aware) wrapping ──────────────────────────────────────

pub(crate) fn adaptive_wrap_line<'a>(line: &'a Line<'a>, base: RtOptions<'a>) -> Vec<Line<'a>> {
    let (flat, span_bounds) = flatten_line(line);
    let mut saw_url = false;
    let mut saw_non_url = false;

    for token in flat.split_ascii_whitespace() {
        if is_url_like_token(token) {
            saw_url = true;
        } else if is_substantive_non_url_token(token) {
            saw_non_url = true;
        }

        if saw_url && saw_non_url {
            break;
        }
    }

    if !saw_url {
        word_wrap_flattened_line(line, &flat, &span_bounds, base)
    } else if saw_non_url {
        mixed_url_wrap_line(line, &flat, &span_bounds, base)
    } else {
        word_wrap_flattened_line(line, &flat, &span_bounds, url_preserving_wrap_options(base))
    }
}

pub(crate) fn adaptive_wrap_lines<'a, I, L>(
    lines: I,
    width_or_options: RtOptions<'a>,
) -> Vec<Line<'static>>
where
    I: IntoIterator<Item = L>,
    L: IntoLineInput<'a>,
{
    let base_opts = width_or_options;
    let mut out: Vec<Line<'static>> = Vec::new();

    for (idx, line) in lines.into_iter().enumerate() {
        let line_input = line.into_line_input();
        let opts = if idx == 0 {
            base_opts.clone()
        } else {
            base_opts
                .clone()
                .initial_indent(base_opts.subsequent_indent.clone())
        };

        let wrapped = adaptive_wrap_line(line_input.as_ref(), opts);
        push_owned_lines(&wrapped, &mut out);
    }

    out
}

// ── Mixed URL/prose wrapping ───────────────────────────────────────────

#[derive(Clone, Debug)]
struct MixedUrlWord {
    range: Range<usize>,
    is_url: bool,
}

impl MixedUrlWord {
    fn width(&self, text: &str) -> usize {
        display_width(&text[self.range.clone()])
    }
}

fn mixed_url_wrap_line<'a>(
    line: &'a Line<'a>,
    flat: &str,
    span_bounds: &[(Range<usize>, ratatui::style::Style)],
    rt_opts: RtOptions<'a>,
) -> Vec<Line<'a>> {
    let initial_width_available = rt_opts
        .width
        .saturating_sub(line_width(&rt_opts.initial_indent))
        .max(1);
    let subsequent_width_available = rt_opts
        .width
        .saturating_sub(line_width(&rt_opts.subsequent_indent))
        .max(1);
    let ranges = mixed_url_wrap_ranges(flat, initial_width_available, subsequent_width_available);

    let mut out = Vec::new();
    for (idx, range) in ranges.iter().enumerate() {
        let mut wrapped_line = if idx == 0 {
            rt_opts.initial_indent.clone()
        } else {
            rt_opts.subsequent_indent.clone()
        }
        .style(line.style);
        let sliced = slice_line_spans(line, span_bounds, range);
        let mut spans = wrapped_line.spans;
        spans.extend(
            sliced
                .spans
                .into_iter()
                .map(|span| span.patch_style(line.style)),
        );
        wrapped_line.spans = spans;
        out.push(wrapped_line);
    }

    if out.is_empty() {
        vec![rt_opts.initial_indent.clone()]
    } else {
        out
    }
}

fn mixed_url_wrap_ranges(
    text: &str,
    initial_width: usize,
    subsequent_width: usize,
) -> Vec<Range<usize>> {
    let leading_space_width = text.chars().take_while(|ch| *ch == ' ').count();
    let mut words = Vec::new();
    let mut cursor = 0usize;
    for word in WordSeparator::AsciiSpace.find_words(text) {
        let word_start = cursor;
        let word_end = word_start + word.word.len();
        let trailing_space_end = word_end + word.whitespace.len();
        if !word.word.is_empty() {
            words.push(MixedUrlWord {
                range: word_start..word_end,
                is_url: is_url_like_token(word.word),
            });
        }
        cursor = trailing_space_end;
    }

    let mut lines = Vec::new();
    let mut line_start = None;
    let mut line_end = 0usize;
    let mut line_width = 0usize;
    let mut line_limit = initial_width.max(1);

    for word in words {
        let mut pending = split_mixed_url_word(text, word, line_limit);
        let mut pending_idx = 0usize;

        while let Some(piece) = pending.get(pending_idx).cloned() {
            let empty_line_prefix_width = if line_start.is_none() && lines.is_empty() {
                leading_space_width
            } else {
                0
            };
            let empty_line_piece_limit = line_limit.saturating_sub(empty_line_prefix_width).max(1);
            let mut indivisible = false;
            if line_start.is_none() && !piece.is_url && piece.width(text) > empty_line_piece_limit {
                let split = split_mixed_url_word(text, piece.clone(), empty_line_piece_limit);
                if split.len() > 1 {
                    pending.splice(pending_idx..=pending_idx, split);
                    continue;
                }
                indivisible = true;
            }

            let piece_width = piece.width(text);
            let inter_word_space = line_start
                .map(|_| text[line_end..piece.range.start].len())
                .unwrap_or(0);
            let fits = if line_start.is_none() {
                piece.is_url
                    || indivisible
                    || empty_line_prefix_width + piece_width <= line_limit
                    || empty_line_prefix_width >= line_limit
            } else {
                line_width + inter_word_space + piece_width <= line_limit
            };

            if fits {
                if line_start.is_none() {
                    let is_first_output_line = lines.is_empty();
                    let start = if is_first_output_line {
                        0
                    } else {
                        piece.range.start
                    };
                    line_start = Some(start);
                    line_width = if is_first_output_line {
                        leading_space_width + piece_width
                    } else {
                        piece_width
                    };
                } else {
                    line_width += inter_word_space + piece_width;
                }
                line_end = piece.range.end;
                pending_idx += 1;
                continue;
            }

            if let Some(start) = line_start.take() {
                lines.push(start..line_end);
            }
            line_end = 0;
            line_width = 0;
            line_limit = subsequent_width.max(1);
        }
    }

    if let Some(start) = line_start {
        lines.push(start..line_end);
    }

    lines
}

fn split_mixed_url_word(text: &str, word: MixedUrlWord, line_limit: usize) -> Vec<MixedUrlWord> {
    if word.is_url || word.width(text) <= line_limit {
        return vec![word];
    }

    let mut pieces = Vec::new();
    let mut start = word.range.start;
    let mut width = 0usize;
    for (offset, grapheme) in text[word.range.clone()].grapheme_indices(true) {
        let grapheme_width = display_width(grapheme);
        if width > 0 && width + grapheme_width > line_limit.max(1) {
            let end = word.range.start + offset;
            pieces.push(MixedUrlWord {
                range: start..end,
                is_url: false,
            });
            start = end;
            width = 0;
        }
        width += grapheme_width;
    }
    if start < word.range.end {
        pieces.push(MixedUrlWord {
            range: start..word.range.end,
            is_url: false,
        });
    }
    pieces
}

// ── Flatten + slice helpers ────────────────────────────────────────────

fn flatten_line(line: &Line<'_>) -> (String, Vec<(Range<usize>, ratatui::style::Style)>) {
    let mut flat = String::new();
    let mut span_bounds = Vec::new();
    let mut acc = 0usize;
    for span in &line.spans {
        let text = span.content.as_ref();
        let start = acc;
        flat.push_str(text);
        acc += text.len();
        span_bounds.push((start..acc, span.style));
    }
    (flat, span_bounds)
}

fn slice_line_spans<'a>(
    original: &'a Line<'a>,
    span_bounds: &[(Range<usize>, ratatui::style::Style)],
    range: &Range<usize>,
) -> Line<'a> {
    let start_byte = range.start;
    let end_byte = range.end;
    let mut acc: Vec<Span<'a>> = Vec::new();
    for (i, (range, style)) in span_bounds.iter().enumerate() {
        let s = range.start;
        let e = range.end;
        if e <= start_byte {
            continue;
        }
        if s >= end_byte {
            break;
        }
        let seg_start = start_byte.max(s);
        let seg_end = end_byte.min(e);
        if seg_end > seg_start {
            let local_start = seg_start - s;
            let local_end = seg_end - s;
            let content = original.spans[i].content.as_ref();
            let slice = &content[local_start..local_end];
            acc.push(Span {
                style: *style,
                content: Cow::Borrowed(slice),
            });
        }
        if e >= end_byte {
            break;
        }
    }
    Line {
        style: original.style,
        alignment: original.alignment,
        spans: acc,
    }
}

// ── IntoLineInput trait ────────────────────────────────────────────────

#[derive(Debug)]
pub(crate) enum LineInput<'a> {
    Borrowed(&'a Line<'a>),
    Owned(Line<'a>),
}

impl<'a> LineInput<'a> {
    fn as_ref(&self) -> &Line<'a> {
        match self {
            LineInput::Borrowed(line) => line,
            LineInput::Owned(line) => line,
        }
    }
}

pub(crate) trait IntoLineInput<'a> {
    fn into_line_input(self) -> LineInput<'a>;
}

impl<'a> IntoLineInput<'a> for &'a Line<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Borrowed(self)
    }
}

impl<'a> IntoLineInput<'a> for &'a mut Line<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Borrowed(self)
    }
}

impl<'a> IntoLineInput<'a> for Line<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(self)
    }
}

impl<'a> IntoLineInput<'a> for String {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for &'a str {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for Cow<'a, str> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for Span<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for Vec<Span<'a>> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}
