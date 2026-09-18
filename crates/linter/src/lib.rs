//! Linter core: the rule contract, the single-pass traversal engine, and the
//! diagnostic model. Rules live in the `rules` crate and reach this crate
//! through [`Rule`]; the engine reaches rules through the generated
//! [`Dispatch`] implementation, so this crate never depends on `rules`.

mod context;
mod diagnostic;
mod engine;
mod rule;

pub use context::Context;
pub use diagnostic::{Applicability, Diagnostic, Edit, Fix, Severity};
pub use engine::{lint_file, lint_parsed, FileResult, SYNTAX_RULE};
pub use rule::{
    ConfigDefault, ConfigOption, Department, Dispatch, FixAvailability, NoRules, Rule, RuleMeta,
    Stability,
};
