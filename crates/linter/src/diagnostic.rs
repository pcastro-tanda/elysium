use std::borrow::Cow;

use ruby_source::Span;

/// RuboCop's severity levels, lowest to highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// `I`
    Info,
    /// `R`
    Refactor,
    /// `C`
    Convention,
    /// `W`
    Warning,
    /// `E`
    Error,
    /// `F`
    Fatal,
}

impl Severity {
    /// One-letter code used by RuboCop's progress and simple formatters.
    pub const fn code(self) -> char {
        match self {
            Self::Info => 'I',
            Self::Refactor => 'R',
            Self::Convention => 'C',
            Self::Warning => 'W',
            Self::Error => 'E',
            Self::Fatal => 'F',
        }
    }

    /// Lower-case name used in RuboCop's JSON output and config files.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Refactor => "refactor",
            Self::Convention => "convention",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Fatal => "fatal",
        }
    }

    /// Parses a RuboCop severity name.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "info" => Self::Info,
            "refactor" => Self::Refactor,
            "convention" => Self::Convention,
            "warning" => Self::Warning,
            "error" => Self::Error,
            "fatal" => Self::Fatal,
            _ => return None,
        })
    }
}

/// Whether a fix can be applied without changing program behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Applicability {
    /// Behaviour-preserving; applied by `fix` by default.
    Safe,
    /// May change behaviour; applied only with `--unsafe`.
    Unsafe,
}

/// One byte-range replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Bytes to replace. Empty span means insertion.
    pub span: Span,
    /// Replacement bytes. Empty means deletion.
    pub replacement: Box<[u8]>,
}

impl Edit {
    /// Replaces `span` with `replacement`.
    pub fn replace(span: Span, replacement: impl Into<Box<[u8]>>) -> Self {
        Self { span, replacement: replacement.into() }
    }

    /// Deletes `span`.
    pub fn delete(span: Span) -> Self {
        Self { span, replacement: Box::default() }
    }

    /// Inserts `text` at `offset`.
    pub fn insert(offset: u32, text: impl Into<Box<[u8]>>) -> Self {
        Self { span: Span::empty(offset), replacement: text.into() }
    }
}

/// A set of edits that together resolve one diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    /// Safety classification.
    pub applicability: Applicability,
    /// Non-overlapping edits, in any order.
    pub edits: Vec<Edit>,
}

/// One reported offense.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// RuboCop cop name, e.g. `Style/FrozenStringLiteralComment`.
    pub rule: &'static str,
    /// Human-readable message.
    pub message: Cow<'static, str>,
    /// Byte range the offense points at.
    pub span: Span,
    /// Effective severity after config overrides.
    pub severity: Severity,
    /// Fix, if the rule produced one for this offense.
    pub fix: Option<Fix>,
}

impl Diagnostic {
    /// Creates a diagnostic without a fix.
    pub fn new(
        rule: &'static str,
        span: Span,
        severity: Severity,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self { rule, message: message.into(), span, severity, fix: None }
    }

    /// Attaches a fix.
    #[must_use]
    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }

    /// True when a fix is attached, regardless of safety.
    pub fn is_correctable(&self) -> bool {
        self.fix.is_some()
    }
}
