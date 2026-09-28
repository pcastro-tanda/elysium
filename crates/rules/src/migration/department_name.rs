//! `Migration/DepartmentName`, ported from RuboCop's
//! `lib/rubocop/cop/migration/department_name.rb`.
//!
//! # Scanning `DISABLE_COMMENT_FORMAT`'s captured tail
//!
//! Upstream scans the text after `# rubocop:disable `/`enable `/`todo ` with
//! `/[^,]+|\W+/`: at each position, either the longest run of non-comma
//! bytes (a content token, which may include a comma-less cop name plus any
//! trailing punctuation attached to it) or, when the position is itself a
//! comma, the longest run of non-word bytes (a separator token: commas plus
//! surrounding spaces). [`scan_tokens`] replays that same two-branch choice
//! byte by byte instead of compiling the alternation as a regex.
//!
//! `valid_content_token?` checks `/\W+/.match?` *unanchored*, so a token
//! that starts with a real cop name but contains any additional
//! non-word byte anywhere (a slash, a space, a stray `:`) is treated as
//! already "valid" (skipped) -- the department-name check only ever fires
//! on a token that is *purely* letters/digits/underscore, exactly a bare
//! cop name with no department. This is why `Style/Alias` (has `/`),
//! `Style:Alias` (has `:`), and `Style` alone (a real department, checked
//! last) all pass, while `Alias` and `LineLength` do not.
//!
//! # `qualified_cop_name` / legacy fallback
//!
//! Upstream asks the live cop registry to qualify the bare name and, if
//! that fails (the name matches nothing registered today), falls back to
//! `RuboCop::ConfigObsoletion.legacy_cop_names` -- the `old_name`s from
//! `config/obsoletion.yml`'s `renamed`/`removed`/`split`/`extracted`
//! sections -- keeping the *legacy* department rather than the cop's
//! current one. [`COP_NAMES`] is derived from the embedded
//! `config::DEFAULT_YML` (mirroring the registry); [`LEGACY_NAMES`] is
//! hardcoded from `config/obsoletion.yml` in file order, since obsoletion
//! data is not part of `config`'s public API.

use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Department name is missing.";

/// RuboCop's `DISABLE_COMMENT_FORMAT`.
static DISABLE_COMMENT_FORMAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A(# *rubocop *: *((dis|en)able|todo) +)(.*)").expect("valid regex")
});

/// Every `Department/CopName` key in the embedded `config/default.yml`,
/// standing in for `Registry.global`'s list of known cops.
static COP_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    config::DEFAULT_YML
        .lines()
        .filter(|line| {
            line.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
                && line.ends_with(':')
                && line.contains('/')
        })
        .map(|line| &line[..line.len() - 1])
        .collect()
});

/// Every department named by a cop in [`COP_NAMES`], standing in for
/// `Registry.global.department?`.
static DEPARTMENTS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut depts: Vec<&'static str> =
        COP_NAMES.iter().filter_map(|cop| cop.split_once('/').map(|(dept, _)| dept)).collect();
    depts.sort_unstable();
    depts.dedup();
    depts
});

/// `old_name`s from `config/obsoletion.yml`'s `renamed`, `removed`, `split`
/// and `extracted` sections, in file order (the order
/// `ConfigObsoletion#legacy_cop_names` -- and so `Array#detect` -- sees
/// them in), standing in for `RuboCop::ConfigObsoletion.legacy_cop_names`.
const LEGACY_NAMES: &[&str] = &[
    "Layout/AlignArguments",
    "Layout/AlignArray",
    "Layout/AlignHash",
    "Layout/AlignParameters",
    "Layout/IndentArray",
    "Layout/IndentAssignment",
    "Layout/IndentFirstArgument",
    "Layout/IndentFirstArrayElement",
    "Layout/IndentFirstHashElement",
    "Layout/IndentFirstParameter",
    "Layout/IndentHash",
    "Layout/IndentHeredoc",
    "Layout/LeadingBlankLines",
    "Layout/Tab",
    "Layout/TrailingBlankLines",
    "Lint/BlockAlignment",
    "Lint/DefEndAlignment",
    "Lint/DuplicatedKey",
    "Lint/EndAlignment",
    "Lint/EndInMethod",
    "Lint/Eval",
    "Lint/HandleExceptions",
    "Lint/MultipleCompare",
    "Lint/StringConversionInInterpolation",
    "Lint/UnneededCopDisableDirective",
    "Lint/UnneededCopEnableDirective",
    "Lint/UnneededRequireStatement",
    "Lint/UnneededSplatExpansion",
    "Metrics/LineLength",
    "Naming/PredicateName",
    "Naming/UncommunicativeBlockParamName",
    "Naming/UncommunicativeMethodParamName",
    "Style/AccessorMethodName",
    "Style/AsciiIdentifiers",
    "Style/ClassAndModuleCamelCase",
    "Style/ConstantName",
    "Style/DeprecatedHashMethods",
    "Style/FileName",
    "Style/FlipFlop",
    "Style/MethodCallParentheses",
    "Style/MethodName",
    "Style/OpMethod",
    "Style/PredicateName",
    "Style/SingleSpaceBeforeFirstArg",
    "Style/UnneededCapitalW",
    "Style/UnneededCondition",
    "Style/UnneededInterpolation",
    "Style/UnneededPercentQ",
    "Style/UnneededSort",
    "Style/VariableName",
    "Style/VariableNumber",
    "Gemspec/DateAssignment",
    "Layout/SpaceAfterControlKeyword",
    "Layout/SpaceBeforeModifierKeyword",
    "Lint/InvalidCharacterLiteral",
    "Lint/RescueWithoutErrorClass",
    "Lint/SpaceBeforeFirstArg",
    "Lint/UselessComparison",
    "Style/BracesAroundHashParameters",
    "Style/MethodMissingSuper",
    "Style/SpaceAfterControlKeyword",
    "Style/SpaceBeforeModifierKeyword",
    "Style/TrailingComma",
    "Style/TrailingCommaInLiteral",
    "Style/MethodMissing",
    "Performance/*",
    "Rails/*",
];

/// RuboCop's `Badge.parse`'s per-segment `camel_case`:
/// `/^[a-z]|_[a-z]/` replaced with the matched letter, uppercased (dropping
/// the underscore, if any).
fn camel_case(part: &str) -> String {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z]|_[a-z]").expect("valid"));
    RE.replace_all(part, |caps: &regex::Captures<'_>| {
        caps[0].chars().last().map(|c| c.to_ascii_uppercase().to_string()).unwrap_or_default()
    })
    .into_owned()
}

/// `Registry#qualified_cop_name` for an unqualified (department-less) name:
/// exactly one registered cop whose name ends in `/<CamelCasedName>` wins;
/// zero or more than one leaves the reference unresolved.
fn qualify_bare(name: &str) -> Option<&'static str> {
    let camelled = camel_case(name);
    let mut matches =
        COP_NAMES.iter().copied().filter(|cop| cop.rsplit('/').next() == Some(camelled.as_str()));
    match (matches.next(), matches.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// `DepartmentName#qualified_legacy_cop_name`: the first legacy name whose
/// bare cop part matches `name` exactly.
fn qualify_legacy(name: &str) -> Option<&'static str> {
    LEGACY_NAMES.iter().copied().find(|legacy| legacy.split('/').nth(1) == Some(name))
}

/// `DepartmentName#check_cop_name`'s corrector: qualify with the live
/// registry, falling back to the legacy name when the registry has nothing
/// (or leaving the name untouched, matching upstream's replace-with-itself
/// when neither resolves).
fn qualify(name: &str) -> String {
    qualify_bare(name).or_else(|| qualify_legacy(name)).unwrap_or(name).to_string()
}

/// RuboCop's `valid_content_token?`.
fn valid_content_token(token: &str) -> bool {
    token.bytes().any(|b| !(b.is_ascii_alphanumeric() || b == b'_'))
        || token.contains("all")
        || DEPARTMENTS.contains(&token)
}

/// RuboCop's `contain_unexpected_character_for_department_name?`.
fn contains_unexpected_character(token: &str) -> bool {
    token.bytes().any(|b| !(b.is_ascii_alphabetic() || b == b'/' || b == b',' || b == b' '))
}

/// Replays Ruby's `/[^,]+|\W+/` global scan over `s`: at a comma, the
/// longest run of non-word bytes; otherwise the longest run of non-comma
/// bytes.
fn scan_tokens(s: &str) -> Vec<&str> {
    fn is_word(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_'
    }
    let bytes = s.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let start = i;
        if bytes[i] == b',' {
            while i < bytes.len() && !is_word(bytes[i]) {
                i += 1;
            }
        } else {
            while i < bytes.len() && bytes[i] != b',' {
                i += 1;
            }
        }
        tokens.push(&s[start..i]);
    }
    tokens
}

/// Check that cop names in rubocop:disable (etc) comments are given with department name.
#[derive(Debug, Clone)]
pub struct DepartmentName;

impl DepartmentName {
    /// `DepartmentName#check_cop_name`.
    fn check_cop_name(ctx: &mut Context<'_>, comment_start: u32, offset: u32, trimmed: &str) {
        let start = comment_start + offset;
        let end = start + u32::try_from(trimmed.len()).expect("cop name fits u32");
        let span = Span::new(start, end);
        let qualified = qualify(trimmed);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, qualified.into_bytes())],
            },
        );
    }
}

impl Rule for DepartmentName {
    const META: RuleMeta = RuleMeta {
        name: "Migration/DepartmentName",
        department: Department::Migration,
        summary:
            "Check that cop names in rubocop:disable (etc) comments are given with department name.",
        explanation: "Check that cop names in rubocop:disable comments are given with \
            department name.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let comments = ctx.comments().to_vec();
        for comment in comments {
            let text = ctx.text(comment.span);
            let Ok(text_str) = std::str::from_utf8(text) else { continue };
            let Some(caps) = DISABLE_COMMENT_FORMAT.captures(text_str) else { continue };
            let prefix_len = u32::try_from(caps.get(1).expect("group 1 present").as_str().len())
                .expect("prefix fits u32");
            let rest = caps.get(4).expect("group 4 present").as_str();

            let mut offset = prefix_len;
            for tok in scan_tokens(rest) {
                let trimmed = tok.trim();
                if !valid_content_token(trimmed) {
                    Self::check_cop_name(ctx, comment.span.start, offset, trimmed);
                }
                if contains_unexpected_character(tok) {
                    break;
                }
                offset += u32::try_from(tok.len()).expect("token fits u32");
            }
        }
    }
}
