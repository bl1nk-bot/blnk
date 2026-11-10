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

    let mut builder = ProjectedTextBuilder::new(text.len());
    for (source_start, grapheme) in text.grapheme_indices(true) {
        if grapheme.contains(['\u{FF9E}', '\u{FF9F}']) {
            builder.push_sound_mark_grapheme(source_start, grapheme);
        } else {
            builder.push_normal_grapheme(source_start, grapheme);
        }
    }

    Some(builder.finish())
}

struct ProjectedTextBuilder {
    projected: String,
    source_boundaries: Vec<(usize, usize)>,
    grapheme_boundaries: Vec<usize>,
}

impl ProjectedTextBuilder {
    fn new(capacity: usize) -> Self {
        Self {
            projected: String::with_capacity(capacity),
            source_boundaries: vec![(0, 0)],
            grapheme_boundaries: vec![0],
        }
    }

    fn push_normal_grapheme(&mut self, source_start: usize, grapheme: &str) {
        for (offset, ch) in grapheme.char_indices() {
            self.projected.push(ch);
            self.source_boundaries
                .push((self.projected.len(), source_start + offset + ch.len_utf8()));
        }
        self.grapheme_boundaries.push(self.projected.len());
    }

    fn push_sound_mark_grapheme(&mut self, source_start: usize, grapheme: &str) {
        let source_end = source_start + grapheme.len();
        let content_start = grapheme
            .find(|ch: char| !ch.is_whitespace())
            .unwrap_or(grapheme.len());
        let (whitespace, content) = grapheme.split_at(content_start);

        for (offset, ch) in whitespace.char_indices() {
            self.projected.push(ch);
            self.source_boundaries
                .push((self.projected.len(), source_start + offset + ch.len_utf8()));
            self.grapheme_boundaries.push(self.projected.len());
        }

        self.project_synthetic_content(display_width(content), source_end);
        self.grapheme_boundaries.push(self.projected.len());
    }

    fn project_synthetic_content(&mut self, width: usize, source_end: usize) {
        let projected_start = self.projected.len();
        for _ in 0..width / 2 {
            self.push_synthetic_char('界', projected_start, source_end);
        }
        if width % 2 == 1 {
            self.push_synthetic_char('a', projected_start, source_end);
        }
    }

    fn push_synthetic_char(&mut self, ch: char, projected_start: usize, source_end: usize) {
        if self.projected.len() > projected_start {
            self.projected.push('\u{2060}');
        }
        self.projected.push(ch);
        self.source_boundaries
            .push((self.projected.len(), source_end));
    }

    fn finish(self) -> ProjectedText {
        ProjectedText {
            text: self.projected,
            source_boundaries: self.source_boundaries,
            grapheme_boundaries: self.grapheme_boundaries,
        }
    }
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
    let mut pieces = Vec::new();

    for word in words {
        if display_width(word.word) <= line_width {
            pieces.push(word);
        } else {
            split_oversized_word(word, projected, line_width, &mut pieces);
        }
    }

    pieces
}

fn split_oversized_word<'a>(
    word: Word<'a>,
    projected: &'a ProjectedText,
    line_width: usize,
    pieces: &mut Vec<Word<'a>>,
) {
    let projected_start = projected.text.as_ptr() as usize;
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
        .take_while(|&atom_end| atom_end <= word_end)
    {
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
    let stripped = strip_synthetic_prefix(wrapped, synthetic_prefix);
    let start = align_line_start(text, cursor, stripped);
    let end = match_wrapped_chars_to_source(text, start, stripped, cursor);
    start..end
}

fn strip_synthetic_prefix<'a>(wrapped: &'a str, synthetic_prefix: &str) -> &'a str {
    if synthetic_prefix.is_empty() {
        wrapped
    } else {
        wrapped.strip_prefix(synthetic_prefix).unwrap_or(wrapped)
    }
}

fn align_line_start(text: &str, mut cursor: usize, wrapped: &str) -> usize {
    if wrapped.starts_with(' ') {
        return cursor;
    }
    while let Some(ch) = text[cursor..].chars().next() {
        if ch != ' ' {
            break;
        }
        cursor += ch.len_utf8();
    }
    cursor
}

fn match_wrapped_chars_to_source(text: &str, start: usize, wrapped: &str, cursor: usize) -> usize {
    let mut end = start;
    let mut saw_source_char = false;
    let mut chars = wrapped.chars().peekable();

    while let Some(ch) = chars.next() {
        if let Some(src) = text.get(end..).and_then(|s| s.chars().next())
            && ch == src
        {
            end += src.len_utf8();
            saw_source_char = true;
            continue;
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

    end
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
    !rest.is_empty() && is_valid_scheme(scheme)
}

fn is_valid_scheme(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    let first_valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    first_valid && chars.all(is_scheme_char)
}

fn is_scheme_char(c: char) -> bool {
    matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '-' | '.')
}

fn is_bare_url_like(token: &str) -> bool {
    let (host_port, has_trailer) = split_host_port_and_trailer(token);
    if !is_candidate_bare_url(host_port, has_trailer) {
        return false;
    }

    let (host, port) = split_host_and_port(host_port);
    if !is_valid_bare_host_and_port(host, port) {
        return false;
    }

    is_supported_host(host)
}

fn is_candidate_bare_url(host_port: &str, has_trailer: bool) -> bool {
    if host_port.is_empty() {
        return false;
    }
    has_trailer || host_port.to_ascii_lowercase().starts_with("www.")
}

fn is_valid_bare_host_and_port(host: &str, port: Option<&str>) -> bool {
    if host.is_empty() {
        return false;
    }
    port.is_none_or(is_valid_port)
}

fn is_supported_host(host: &str) -> bool {
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

    let Some((host, port)) = host_port.rsplit_once(':') else {
        return (host_port, None);
    };

    if is_valid_host_port_pair(host, port) {
        (host, Some(port))
    } else {
        (host_port, None)
    }
}

fn is_valid_host_port_pair(host: &str, port: &str) -> bool {
    !host.is_empty() && is_numeric_port(port)
}

fn is_numeric_port(port: &str) -> bool {
    !port.is_empty() && port.chars().all(|c| c.is_ascii_digit())
}

fn is_valid_port(port: &str) -> bool {
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
    let words = collect_mixed_url_words(text);
    let mut builder = MixedUrlLineBuilder::new(text, initial_width, subsequent_width);
    for word in words {
        builder.push_word(word);
    }
    builder.finish()
}

fn collect_mixed_url_words(text: &str) -> Vec<MixedUrlWord> {
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
    words
}

struct MixedUrlLineBuilder<'a> {
    text: &'a str,
    leading_space_width: usize,
    subsequent_width: usize,
    lines: Vec<Range<usize>>,
    line_start: Option<usize>,
    line_end: usize,
    line_width: usize,
    line_limit: usize,
}

impl<'a> MixedUrlLineBuilder<'a> {
    fn new(text: &'a str, initial_width: usize, subsequent_width: usize) -> Self {
        let leading_space_width = text.chars().take_while(|ch| *ch == ' ').count();
        Self {
            text,
            leading_space_width,
            subsequent_width,
            lines: Vec::new(),
            line_start: None,
            line_end: 0,
            line_width: 0,
            line_limit: initial_width.max(1),
        }
    }

    fn push_word(&mut self, word: MixedUrlWord) {
        let mut pending = split_mixed_url_word(self.text, word, self.line_limit);
        let mut pending_idx = 0usize;

        while let Some(piece) = pending.get(pending_idx).cloned() {
            let (should_retry, indivisible) =
                self.try_resplit_empty_line_piece(&piece, &mut pending, pending_idx);
            if should_retry {
                continue;
            }

            if self.piece_fits(&piece, indivisible) {
                self.append_piece(&piece);
                pending_idx += 1;
            } else {
                self.flush_line();
            }
        }
    }

    fn is_empty_line(&self) -> bool {
        self.line_start.is_none()
    }

    fn is_first_line(&self) -> bool {
        self.lines.is_empty()
    }

    fn empty_line_prefix_width(&self) -> usize {
        if self.is_empty_line() && self.is_first_line() {
            self.leading_space_width
        } else {
            0
        }
    }

    fn empty_line_piece_limit(&self) -> usize {
        self.line_limit
            .saturating_sub(self.empty_line_prefix_width())
            .max(1)
    }

    fn can_resplit_piece(&self, piece: &MixedUrlWord) -> bool {
        self.is_empty_line() && !piece.is_url
    }

    fn try_resplit_empty_line_piece(
        &self,
        piece: &MixedUrlWord,
        pending: &mut Vec<MixedUrlWord>,
        pending_idx: usize,
    ) -> (bool, bool) {
        if !self.can_resplit_piece(piece) {
            return (false, false);
        }

        let limit = self.empty_line_piece_limit();
        if piece.width(self.text) <= limit {
            return (false, false);
        }

        let split = split_mixed_url_word(self.text, piece.clone(), limit);
        if split.len() > 1 {
            pending.splice(pending_idx..=pending_idx, split);
            (true, false)
        } else {
            (false, true)
        }
    }

    fn piece_fits(&self, piece: &MixedUrlWord, indivisible: bool) -> bool {
        if self.is_empty_line() {
            self.empty_line_piece_fits(piece, indivisible)
        } else {
            self.continuing_line_piece_fits(piece)
        }
    }

    fn empty_line_piece_fits(&self, piece: &MixedUrlWord, indivisible: bool) -> bool {
        if piece.is_url || indivisible {
            return true;
        }
        let total_width = self.empty_line_prefix_width() + piece.width(self.text);
        total_width <= self.line_limit || self.empty_line_prefix_width() >= self.line_limit
    }

    fn continuing_line_piece_fits(&self, piece: &MixedUrlWord) -> bool {
        let inter_word_space = self.text[self.line_end..piece.range.start].len();
        self.line_width + inter_word_space + piece.width(self.text) <= self.line_limit
    }

    fn append_piece(&mut self, piece: &MixedUrlWord) {
        let piece_width = piece.width(self.text);
        if self.is_empty_line() {
            let start = if self.is_first_line() {
                0
            } else {
                piece.range.start
            };
            self.line_start = Some(start);
            self.line_width = if self.is_first_line() {
                self.leading_space_width + piece_width
            } else {
                piece_width
            };
        } else {
            let inter_word_space = self.text[self.line_end..piece.range.start].len();
            self.line_width += inter_word_space + piece_width;
        }
        self.line_end = piece.range.end;
    }

    fn flush_line(&mut self) {
        if let Some(start) = self.line_start.take() {
            self.lines.push(start..self.line_end);
        }
        self.line_end = 0;
        self.line_width = 0;
        self.line_limit = self.subsequent_width.max(1);
    }

    fn finish(mut self) -> Vec<Range<usize>> {
        if let Some(start) = self.line_start {
            self.lines.push(start..self.line_end);
        }
        self.lines
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_scheme_prefix_checks() {
        assert!(has_valid_scheme_prefix("https://example.com"));
        assert!(has_valid_scheme_prefix("http://localhost:8080"));
        assert!(has_valid_scheme_prefix("git+ssh://github.com/repo"));
        assert!(has_valid_scheme_prefix("custom-scheme.1://host"));

        assert!(!has_valid_scheme_prefix("://missing-scheme"));
        assert!(!has_valid_scheme_prefix("https://"));
        assert!(!has_valid_scheme_prefix("123://numeric-start"));
        assert!(!has_valid_scheme_prefix("+invalid://start"));
        assert!(!has_valid_scheme_prefix("plain text"));
    }
}
