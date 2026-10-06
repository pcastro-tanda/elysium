//! `Rails/SafeNavigation`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/safe_navigation.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use Ruby's safe navigation operator (`&.`) instead of `try!`.
#[derive(Debug, Clone)]
pub struct SafeNavigation {
    supported: bool,
    convert_try: bool,
    ignored: Vec<Span>,
}

impl Rule for SafeNavigation {
    const META: RuleMeta = RuleMeta {
        name: "Rails/SafeNavigation",
        department: Department::Rails,
        summary: "Use Ruby's safe navigation operator (`&.`) instead of `try!`.",
        explanation: "Converts usages of `try!` to `&.`. It can also be configured to convert \
                      `try`. It will convert code to use safe navigation if the target Ruby \
                      version is set to 2.3+.\n\n```ruby\n# bad\nfoo.try!(:bar)\n\
                      foo.try!(:bar, baz)\nfoo.try!(:bar) { |e| e.baz }\nfoo.try!(&:bar)\n\n\
                      # good\nfoo.try(:bar)\nfoo&.bar\nfoo&.bar(baz)\nfoo&.bar { |e| e.baz }\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "ConvertTry",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Also convert usages of `try`, not only `try!`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            supported: options.target_ruby_version() >= 2.3,
            convert_try: options.bool("ConvertTry"),
            ignored: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name();
        let try_method = match name.as_slice() {
            b"try" => "try",
            b"try!" => "try!",
            _ => return,
        };
        if try_method == "try" && !self.convert_try {
            return;
        }

        // `(send _ ${:try :try!} $_ ...)`: a first argument (possibly the block pass).
        let block_pass = call.block().and_then(|b| b.as_block_argument_node());
        let first_args: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        let dispatch_is_sym = match first_args.first() {
            Some(first) => first.as_symbol_node().is_some(),
            None => match &block_pass {
                Some(bp) => bp.expression().is_some_and(|e| e.as_symbol_node().is_some()),
                None => return,
            },
        };
        if !dispatch_is_sym {
            return;
        }

        let node_span = {
            let base = call_span_excluding_block(&call);
            match &block_pass {
                Some(bp) => Span::new(base.start, base.end.max(bp.as_node().span().end)),
                None => base,
            }
        };
        if self.ignored.iter().any(|i| node_span.start >= i.start && node_span.end <= i.end) {
            return;
        }

        // Positional parameters after the dispatch argument, as sources.
        let mut args_sources: Vec<String> = Vec::new();
        let replacement = if first_args.is_empty() {
            let bp = block_pass.as_ref().expect("checked above");
            // `(block_pass (sym $_))`: the symbol's name.
            let sym = bp.expression().expect("symbol proc");
            let sym = sym.as_symbol_node().expect("symbol proc");
            let value = String::from_utf8_lossy(sym.unescaped()).into_owned();
            format!("&.{value}")
        } else {
            let method_source =
                String::from_utf8_lossy(ctx.text(first_args[0].span())).into_owned();
            let method = method_source.get(1..).unwrap_or("").to_owned();
            for arg in &first_args[1..] {
                args_sources.push(String::from_utf8_lossy(ctx.text(arg.span())).into_owned());
            }
            if let Some(bp) = &block_pass {
                args_sources
                    .push(String::from_utf8_lossy(ctx.text(bp.as_node().span())).into_owned());
            }
            let new_params = args_sources.join(", ");
            if is_setter_method(&method) {
                format!("&.{} = {new_params}", &method[..method.len() - 1])
            } else if args_sources.is_empty() {
                format!("&.{method}")
            } else {
                format!("&.{method}({new_params})")
            }
        };

        let edit = if let Some(op) = call.call_operator_loc() {
            Edit::replace(Span::new(op.span().start, node_span.end), replacement.into_bytes())
        } else {
            Edit::replace(node_span, format!("self{replacement}").into_bytes())
        };
        let message = format!("Use safe navigation (`&.`) instead of `{try_method}`.");
        ctx.report_with_fix(
            &Self::META,
            node_span,
            message,
            Fix { applicability: Applicability::Safe, edits: vec![edit] },
        );
        self.ignored.push(node_span);
    }
}

/// `method.match?(/\A\w+=\z/)`.
fn is_setter_method(method: &str) -> bool {
    method.strip_suffix('=').is_some_and(|stem| {
        !stem.is_empty() && stem.chars().all(|c| c.is_alphanumeric() || c == '_')
    })
}
