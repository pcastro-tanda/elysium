//! `Lint/EmptyClass`, ported from RuboCop's
//! `lib/rubocop/cop/lint/empty_class.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt, NodeKind};
use ruby_source::Span;

const CLASS_MSG: &str = "Empty class detected.";
const METACLASS_MSG: &str = "Empty metaclass detected.";

/// Checks for classes and metaclasses without a body.
#[derive(Debug, Clone)]
pub struct EmptyClass {
    allow_comments: bool,
}

impl Rule for EmptyClass {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EmptyClass",
        department: Department::Lint,
        summary: "Checks for classes and metaclasses without a body.",
        explanation: "\
Checks for classes and metaclasses without a body.
Such empty classes and metaclasses are typically an oversight or we should provide a comment
to be clearer what we're aiming for.

```ruby
# bad
class Foo
end

class Bar
  class << self
  end
end

class << obj
end

# good
class Foo
  def do_something
    # ... code
  end
end

class Bar
  class << self
    attr_reader :bar
  end
end

class << obj
  attr_reader :bar
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::SingletonClassNode],
        config: &[ConfigOption {
            name: "AllowComments",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Whether a class or metaclass whose body is only comments counts as empty.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_comments: options.bool("AllowComments") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::ClassNode { .. } => {
                let class = node.as_class_node().expect("kind matched");
                if class.superclass().is_some() {
                    return;
                }
                if self.body_or_allowed_comment_lines(ctx, class.body(), node.span()) {
                    return;
                }
                ctx.report(&Self::META, node.span(), CLASS_MSG);
            }
            Node::SingletonClassNode { .. } => {
                let sclass = node.as_singleton_class_node().expect("kind matched");
                if self.body_or_allowed_comment_lines(ctx, sclass.body(), node.span()) {
                    return;
                }
                ctx.report(&Self::META, node.span(), METACLASS_MSG);
            }
            _ => {}
        }
    }
}

impl EmptyClass {
    /// RuboCop's `body_or_allowed_comment_lines?`.
    fn body_or_allowed_comment_lines(
        &self,
        ctx: &Context<'_>,
        body: Option<Node<'_>>,
        span: Span,
    ) -> bool {
        if body.is_some() {
            return true;
        }
        self.allow_comments && contains_comment(ctx, span)
    }
}

/// RuboCop's `ProcessedSource#contains_comment?`: any comment whose line
/// falls within `span`'s first..last physical line.
fn contains_comment(ctx: &Context<'_>, span: Span) -> bool {
    let first_line = ctx.line_col(span.start).line;
    let last_line = ctx.last_line(span);
    ctx.comments().iter().any(|comment| {
        let line = ctx.line_col(comment.span.start).line;
        (first_line..=last_line).contains(&line)
    })
}
