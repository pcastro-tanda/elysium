//! Linter core: the rule contract, the single-pass traversal engine, and the
//! diagnostic model. Rules live in the `rules` crate and reach this crate
//! through [`Rule`]; the engine reaches rules through the generated
//! [`Dispatch`] implementation, so this crate never depends on `rules`.

mod context;
mod diagnostic;
mod engine;
mod fix;
mod options;
mod rule;
mod settings;

pub use context::{CommentInfo, Context};
pub use diagnostic::{Applicability, Diagnostic, Edit, Fix, Severity};
pub use engine::{lint_file, lint_parsed, lint_parsed_with, FileResult, SYNTAX_RULE};
pub use fix::{apply_fixes, fix_file, FixOutcome, FixReport, MAX_FIX_ITERATIONS};
pub use options::{OptionError, OptionValue, PeerOptions, RuleOptions};
pub use rule::{
    subscription_table, ConfigDefault, ConfigOption, Department, Dispatch, FixAvailability,
    NoRules, Rule, RuleMeta, Stability,
};
pub use settings::{intern_rule_name, FileSettings};
