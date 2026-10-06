//! `Rails/WhereExists`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/where_exists.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::util::{parser_args, send_span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Exists,
    Where,
}

/// Prefer `exists?(...)` over `where(...).exists?`.
#[derive(Debug, Clone)]
pub struct WhereExists {
    style: Style,
}

impl Rule for WhereExists {
    const META: RuleMeta = RuleMeta {
        name: "Rails/WhereExists",
        department: Department::Rails,
        summary: "Prefer `exists?(...)` over `where(...).exists?`.",
        explanation: "Enforces consistent style when using `exists?`.\n\nWhen EnforcedStyle is \
                      'exists' the cop enforces `exists?(...)` over `where(...).exists?`; when \
                      'where' it enforces `where(...).exists?` over `exists?(...)`.\n\n```ruby\n\
                      # EnforcedStyle: exists (default)\n# bad\nUser.where(name: \
                      'john').exists?\nUser.where(['name = ?', 'john']).exists?\nUser.where('name \
                      = ?', 'john').exists?\nuser.posts.where(published: true).exists?\n\n# \
                      good\nUser.exists?(name: 'john')\nUser.where('length(name) > \
                      10').exists?\nuser.posts.exists?(published: true)\n```\n\nUnsafe \
                      autocorrection: `Author.includes(:articles).where(articles: {id: \
                      id}).exists?` performs `eager_load` behavior, while \
                      `Author.includes(:articles).exists?(articles: {id: id})` performs \
                      `preload` behavior and raises `ActiveRecord::StatementInvalid`.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("exists"),
            allowed: &["exists", "where"],
            doc: "Whether to prefer `exists?(...)` or `where(...).exists?`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "where" => Style::Where,
            _ => Style::Exists,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"exists?" {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let selector = selector.span();

        let (range, good_method) = match self.style {
            Style::Exists => {
                // (call (call _ :where $...) :exists?)
                if !parser_args(&call).is_empty() {
                    return;
                }
                let Some(receiver) = call.receiver() else { return };
                let Some(inner) = receiver.as_call_node() else { return };
                if inner.name().as_slice() != b"where"
                    || inner.block().is_some_and(|b| b.as_block_node().is_some())
                {
                    return;
                }
                let Some(inner_selector) = inner.message_loc() else { return };
                let args = parser_args(&inner);
                if !convertable_args(&args) {
                    return;
                }
                let range = Span::new(inner_selector.span().start, selector.end);
                let sources: Vec<&str> = args.iter().map(|a| source(ctx, a)).collect();
                let good = if args.len() > 1 {
                    format!("exists?([{}])", sources.join(", "))
                } else {
                    format!("exists?({})", sources[0])
                };
                (range, good)
            }
            Style::Where => {
                // (call _ :exists? $!splat_type?)
                let args = parser_args(&call);
                let [arg] = args.as_slice() else { return };
                if arg.as_splat_node().is_some() || !convertable_args(&args) {
                    return;
                }
                let range = Span::new(selector.start, send_span(&call).end);
                let dot = call
                    .call_operator_loc()
                    .map_or("." , |l| std::str::from_utf8(ctx.text(l.span())).unwrap_or("."));
                (range, format!("where({}){dot}exists?", source(ctx, arg)))
            }
        };

        let bad_method = String::from_utf8_lossy(ctx.text(range)).into_owned();
        let message = format!("Prefer `{good_method}` over `{bad_method}`.");
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, good_method.into_bytes())],
            },
        );
    }
}

fn source<'a>(ctx: &'a Context<'_>, node: &Node<'_>) -> &'a str {
    std::str::from_utf8(ctx.text(node.span())).unwrap_or("")
}

fn convertable_args(args: &[Node<'_>]) -> bool {
    match args {
        [] => false,
        [one] => {
            one.as_hash_node().is_some()
                || one.as_keyword_hash_node().is_some()
                || one.as_array_node().is_some()
        }
        _ => true,
    }
}
