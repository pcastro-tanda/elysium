//! Every rule, one file per rule, grouped by RuboCop department. The
//! [`RuleSet`] dispatcher is generated from the rule declarations at build
//! time (Phase 3); until the first rule lands it is an empty static
//! dispatcher so the pipeline can be exercised end to end.

use linter::{Context, Dispatch};
use ruby_ast::{Node, NodeKind};

/// The configured set of enabled rules for one effective configuration.
#[derive(Debug, Clone, Default)]
pub struct RuleSet;

impl RuleSet {
    /// Every rule enabled with RuboCop's defaults.
    pub fn rubocop_defaults() -> Self {
        Self
    }
}

impl Dispatch for RuleSet {
    #[inline]
    fn file_start(&mut self, _ctx: &mut Context<'_>) {}

    #[inline]
    fn enter(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}

    #[inline]
    fn leave(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}

    #[inline]
    fn file_end(&mut self, _ctx: &mut Context<'_>) {}
}
