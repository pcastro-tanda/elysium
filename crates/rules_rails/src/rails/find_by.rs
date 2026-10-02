//! `Rails/FindBy`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/find_by.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// Prefer `find_by` over `where.first`/`where.take`.
#[derive(Debug, Clone)]
pub struct FindBy {
    ignore_where_first: bool,
}

impl Rule for FindBy {
    const META: RuleMeta = RuleMeta {
        name: "Rails/FindBy",
        department: Department::Rails,
        summary: "Prefer find_by over where.first.",
        explanation: "Identifies usages of `where.take` and change them to use `find_by` \
                      instead.\n\nAnd `where(...).first` can return different results from \
                      `find_by`. (They order records differently, so the \"first\" record can \
                      be different.)\n\nIf you also want to detect `where.first`, you can set \
                      `IgnoreWhereFirst` to false.\n\n```ruby\n# bad\nUser.where(name: \
                      'Bruce').take\n\n# good\nUser.find_by(name: 'Bruce')\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "IgnoreWhereFirst",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Do not flag `where.first`, which orders differently from `find_by`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { ignore_where_first: options.bool("IgnoreWhereFirst") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let is_first = name.as_slice() == b"first";
        if !is_first && name.as_slice() != b"take" {
            return;
        }
        // `node.arguments.empty?`: an `&block` argument counts as an argument.
        if call.arguments().is_some() || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
        {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(where_call) = receiver.as_call_node() else { return };
        // `any_block_type?` receivers are rejected; a `where` method only.
        if where_call.name().as_slice() != b"where"
            || where_call.block().is_some_and(|b| b.as_block_node().is_some())
        {
            return;
        }
        if self.ignore_where_first && is_first {
            return;
        }
        let (Some(where_loc), Some(selector)) = (where_call.message_loc(), call.message_loc())
        else {
            return;
        };
        let range = Span::new(where_loc.span().start, selector.span().end);
        let dot = call.call_operator_loc().map_or(&b""[..], |loc| ctx.text(loc.span()));
        let message = format!(
            "Use `find_by` instead of `where{}{}`.",
            String::from_utf8_lossy(dot),
            String::from_utf8_lossy(name.as_slice())
        );
        if is_first {
            ctx.report(&Self::META, range, message);
            return;
        }
        let where_end = call_span_excluding_block(&where_call).end;
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(where_loc.span(), b"find_by".to_vec()),
                    Edit::delete(Span::new(where_end, selector.span().end)),
                ],
            },
        );
    }
}
