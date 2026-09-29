//! `Lint/IneffectiveAccessModifier`, ported from RuboCop's
//! `lib/rubocop/cop/lint/ineffective_access_modifier.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{each_descendant, node::CallNode, LocationExt as _, Node, NodeExt as _, NodeKind};

const ALTERNATIVE_PRIVATE: &str =
    "`private_class_method` or `private` inside a `class << self` block";
const ALTERNATIVE_PROTECTED: &str = "`protected` inside a `class << self` block";

/// Checks for attempts to use `private` or `protected` to set the visibility of a class method, which does not work.
#[derive(Debug, Clone)]
pub struct IneffectiveAccessModifier;

impl Rule for IneffectiveAccessModifier {
    const META: RuleMeta = RuleMeta {
        name: "Lint/IneffectiveAccessModifier",
        department: Department::Lint,
        summary: "Checks for attempts to use `private` or `protected` to set the visibility of a class method, which does not work.",
        explanation: "\
`private` or `protected` access modifiers which are applied to a singleton
method do not make singleton methods private/protected. `private_class_method`
can be used for that.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::ModuleNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let body = match node.kind() {
            NodeKind::ClassNode => node.as_class_node().and_then(|c| c.body()),
            NodeKind::ModuleNode => node.as_module_node().and_then(|m| m.body()),
            _ => return,
        };
        let Some(body) = body else { return };
        // RuboCop only descends when the body is itself a `:begin` node,
        // i.e. more than one top-level statement (a single statement is
        // never wrapped, so it can never contain a preceding modifier).
        let Some(statements) = body.as_statements_node() else { return };
        let items: Vec<Node<'_>> = statements.body().iter().collect();
        if items.len() < 2 {
            return;
        }
        check_statements(&items, &body, &mut None, &mut None, ctx);
    }
}

/// Port of `ineffective_modifier`: walks the statements of a class/module
/// body (or a nested bare `begin...end`), tracking the last-seen access
/// modifier call and reporting `defs` nodes it ineffectively "protects".
fn check_statements<'pr>(
    items: &[Node<'pr>],
    search_root: &Node<'pr>,
    ignored_methods: &mut Option<Vec<Vec<u8>>>,
    modifier: &mut Option<CallNode<'pr>>,
    ctx: &mut Context<'_>,
) {
    for child in items {
        match child.kind() {
            NodeKind::CallNode => {
                let call = child.as_call_node().expect("kind matched");
                if is_access_modifier(&call) {
                    *modifier = Some(call);
                }
            }
            NodeKind::DefNode => {
                let def = child.as_def_node().expect("kind matched");
                if def.receiver().is_none() {
                    continue;
                }
                if ignored_methods.is_none() {
                    *ignored_methods = Some(private_class_method_names(search_root));
                }
                if correct_visibility(
                    modifier.as_ref(),
                    def.name().as_slice(),
                    ignored_methods.as_ref(),
                ) {
                    continue;
                }
                let Some(modifier) = modifier.as_ref() else { continue };
                let message = format_message(modifier, ctx);
                ctx.report(&IneffectiveAccessModifier::META, def.def_keyword_loc().span(), message);
            }
            NodeKind::BeginNode => {
                let begin = child.as_begin_node().expect("kind matched");
                if begin.begin_keyword_loc().is_none() {
                    // A method body wrapped in a `BeginNode` for `rescue`,
                    // not a bare `begin...end` (`kwbegin`); upstream never
                    // sees these as children of a class/module body.
                    continue;
                }
                if ignored_methods.is_none() {
                    *ignored_methods = Some(private_class_method_names(search_root));
                }
                let nested: Vec<Node<'pr>> =
                    begin.statements().map(|s| s.body().iter().collect()).unwrap_or_default();
                // The recursive call receives the *current* modifier by
                // value; changes made inside it never leak back out.
                let mut nested_modifier = *modifier;
                check_statements(&nested, child, ignored_methods, &mut nested_modifier, ctx);
            }
            _ => {}
        }
    }
}

fn is_access_modifier(call: &CallNode<'_>) -> bool {
    ruby_ast::ext::is_bare_access_modifier(call) && call.name().as_slice() != b"module_function"
}

fn correct_visibility(
    modifier: Option<&CallNode<'_>>,
    method_name: &[u8],
    ignored_methods: Option<&Vec<Vec<u8>>>,
) -> bool {
    let Some(modifier) = modifier else { return true };
    if modifier.name().as_slice() == b"public" {
        return true;
    }
    ignored_methods.is_some_and(|names| names.iter().any(|n| n.as_slice() == method_name))
}

/// Port of `private_class_method_names`: every literal (symbol/string)
/// argument to a bare `private_class_method` call anywhere in `node`'s
/// subtree.
fn private_class_method_names(node: &Node<'_>) -> Vec<Vec<u8>> {
    let mut names = Vec::new();
    each_descendant(node, &mut |descendant| {
        let Some(call) = descendant.as_call_node() else { return };
        if call.receiver().is_some() || call.name().as_slice() != b"private_class_method" {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        for arg in &arguments.arguments() {
            if let Some(sym) = arg.as_symbol_node() {
                names.push(sym.unescaped().to_vec());
            } else if let Some(s) = arg.as_string_node() {
                names.push(s.unescaped().to_vec());
            }
        }
    });
    names
}

fn format_message(modifier: &CallNode<'_>, ctx: &Context<'_>) -> String {
    let visibility = modifier.name().as_slice();
    let visibility_str = String::from_utf8_lossy(visibility);
    let alternative =
        if visibility == b"private" { ALTERNATIVE_PRIVATE } else { ALTERNATIVE_PROTECTED };
    let line = ctx.line_col(modifier.as_node().span().start).line;
    format!(
        "`{visibility_str}` (on line {line}) does not make singleton methods {visibility_str}. Use {alternative} instead."
    )
}
