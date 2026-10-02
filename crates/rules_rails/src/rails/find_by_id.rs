//! `Rails/FindById`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/find_by_id.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::util;

/// Favor `find` over `where.take!`, `find_by!` and `find_by_id!`.
#[derive(Debug, Clone)]
pub struct FindById;

impl Rule for FindById {
    const META: RuleMeta = RuleMeta {
        name: "Rails/FindById",
        department: Department::Rails,
        summary: "Favor the use of `find` over `where.take!`, `find_by!`, and `find_by_id!` \
                  when you need to retrieve a single record by primary key when you expect it \
                  to be found.",
        explanation: "Enforces that `ActiveRecord#find` is used instead of `where.take!`, \
                      `find_by!`, and `find_by_id!` to retrieve a single record by primary key \
                      when you expect it to be found.\n\n```ruby\n# bad\nUser.where(id: \
                      id).take!\nUser.find_by_id!(id)\nUser.find_by!(id: id)\n\n# good\n\
                      User.find(id)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        let Some(selector) = call.message_loc() else { return };
        let args = util::parser_args(&call);
        match name {
            b"take!" => {
                if !args.is_empty() {
                    return;
                }
                let Some(receiver) = call.receiver() else { return };
                let Some(where_call) = receiver.as_call_node() else { return };
                if where_call.name().as_slice() != b"where" {
                    return;
                }
                let where_args = util::parser_args(&where_call);
                let [hash] = where_args.as_slice() else { return };
                let Some(id_value) = util::sole_id_pair_value(hash) else { return };
                let Some(where_selector) = where_call.message_loc() else { return };
                let range = Span::new(where_selector.span().start, node.span().end);
                register_offense(ctx, range, id_value.span());
            }
            b"find_by_id!" => {
                let [id_value] = args.as_slice() else { return };
                let range = Span::new(selector.span().start, node.span().end);
                register_offense(ctx, range, id_value.span());
            }
            b"find_by!" => {
                let [hash] = args.as_slice() else { return };
                let Some(id_value) = util::sole_id_pair_value(hash) else { return };
                let range = Span::new(selector.span().start, node.span().end);
                register_offense(ctx, range, id_value.span());
            }
            _ => {}
        }
    }
}

fn register_offense(ctx: &mut Context<'_>, range: Span, id_value: Span) {
    let good_method = format!("find({})", String::from_utf8_lossy(ctx.text(id_value)));
    let message =
        format!("Use `{good_method}` instead of `{}`.", String::from_utf8_lossy(ctx.text(range)));
    ctx.report_with_fix(
        &FindById::META,
        range,
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(range, good_method.into_bytes())],
        },
    );
}
