//! Errors, with the place in the app's source they are about.

use std::{fmt, ops::Range};

/// What is wrong with a shader: a WGSL error naga reports, a rule of NSH's profile the shader breaks
/// (`docs/dev/shaders.md` "The profile"), or a limit it goes over.
///
/// `span` is a byte range in the source the app wrote (never in NSH's prelude), `line` and `column`
/// (1-based) where it starts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Error {
    /// What is wrong, one sentence.
    pub message: String,
    /// The bytes of the source it is about, when it is about a place.
    pub span: Option<Range<usize>>,
    /// The line `span` starts on, 1-based (0 without a span).
    pub line: u32,
    /// The column `span` starts at, 1-based, in characters (0 without a span).
    pub column: u32,
    /// What the place is (naga's label), or how to fix it.
    pub notes: Vec<String>,
}

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ..Default::default()
        }
    }

    /// An error about `span` of `source`; a span outside the app's source (NSH's prelude) is dropped.
    pub(crate) fn at(message: impl Into<String>, source: &str, span: Option<Range<usize>>) -> Self {
        let mut error = Self::new(message);
        error.locate(source, span);
        error
    }

    pub(crate) fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    fn locate(&mut self, source: &str, span: Option<Range<usize>>) {
        let Some(span) = span.filter(|span| span.end <= source.len() && span.start <= span.end)
        else {
            return;
        };
        let before = &source[..span.start];
        self.line = before.matches('\n').count() as u32 + 1;
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        self.column = source[line_start..span.start].chars().count() as u32 + 1;
        self.span = Some(span);
    }

    /// The error as a compiler prints it: `path:line:column: message`, the line of source with the
    /// span underlined, then the notes.
    pub fn render(&self, path: &str, source: &str) -> String {
        let mut out = String::new();
        match self.span {
            Some(ref span) => {
                out.push_str(&format!(
                    "{path}:{}:{}: {}\n",
                    self.line, self.column, self.message
                ));
                let line_start = source[..span.start].rfind('\n').map_or(0, |i| i + 1);
                let line_end = source[span.start..]
                    .find('\n')
                    .map_or(source.len(), |i| span.start + i);
                let text = &source[line_start..line_end];
                let gutter = self.line.to_string();
                out.push_str(&format!("{gutter} | {text}\n"));
                let width = source[span.start..span.end.min(line_end)]
                    .chars()
                    .count()
                    .max(1);
                out.push_str(&format!(
                    "{} | {}{}\n",
                    " ".repeat(gutter.len()),
                    " ".repeat(self.column as usize - 1),
                    "^".repeat(width)
                ));
            }
            None => out.push_str(&format!("{path}: {}\n", self.message)),
        }
        for note in &self.notes {
            out.push_str(&format!("  = {note}\n"));
        }
        out
    }

    /// naga's WGSL parse error; its first label in the app's source is the place.
    pub(crate) fn from_parse(error: &naga::front::wgsl::ParseError, source: &str) -> Self {
        let mut labels = error.labels().filter_map(|(span, label)| {
            let range = span.to_range()?;
            (range.end <= source.len()).then_some((range, label))
        });
        let mut out = match labels.next() {
            Some((range, label)) => {
                let mut out = Self::at(error.message(), source, Some(range));
                if !label.is_empty() {
                    out.notes.push(label.to_string());
                }
                out
            }
            None => Self::new(error.message()),
        };
        out.notes.extend(error.notes().map(str::to_string));
        out
    }

    /// naga's validation error: the error and its causes in one message, the first span in the
    /// app's source as the place.
    pub(crate) fn from_validation(
        error: &naga::WithSpan<naga::valid::ValidationError>,
        source: &str,
    ) -> Self {
        let mut message = error.as_inner().to_string();
        let mut cause = std::error::Error::source(error.as_inner());
        while let Some(inner) = cause {
            message.push_str(": ");
            message.push_str(&inner.to_string());
            cause = inner.source();
        }
        let span = error
            .spans()
            .find_map(|(span, _)| span.to_range().filter(|range| range.end <= source.len()));
        Self::at(message, source, span)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.span.is_some() {
            write!(f, "{}:{}: ", self.line, self.column)?;
        }
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}
