//! `Layout/EmptyLinesAroundModuleBody`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_lines_around_module_body.rb` plus the
//! `EmptyLinesAroundBody` mixin it includes (shared logic lives in
//! `empty_lines_around_body.rs`).
//!
//! Unlike `Layout/EmptyLinesAroundClassBody`, this cop's config never
//! exposes the `beginning_only`/`ending_only` styles, only `no_empty_lines`
//! (default), `empty_lines`, `empty_lines_except_namespace`, and
//! `empty_lines_special` -- see `config/default.yml`.
//!
//! Prism always wraps a multi-statement body in a
//! [`ruby_ast::NodeKind::StatementsNode`], even when it holds exactly one
//! statement -- unlike the whitequark AST RuboCop's source is written
//! against, where a single-statement body is that statement node directly.
//! `empty_lines_around_body::shape_of` renormalizes this.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

use super::empty_lines_around_body::{
    check_beginning, check_both, check_deferred_empty_line, check_ending,
    first_child_requires_empty_line, is_namespace, shape_of, Want,
};

/// RuboCop's `EnforcedStyle` for this cop (a subset of the class body cop's
/// six styles: no `beginning_only`/`ending_only`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    NoEmptyLines,
    EmptyLines,
    EmptyLinesExceptNamespace,
    EmptyLinesSpecial,
}

/// Looks for empty lines around the bodies of `module` and matches them
/// against the configured style.
#[derive(Debug, Clone)]
pub struct EmptyLinesAroundModuleBody {
    style: Style,
}

impl Rule for EmptyLinesAroundModuleBody {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLinesAroundModuleBody",
        department: Department::Layout,
        summary: "Keeps track of empty lines around module bodies.",
        explanation: "\
```ruby
# EnforcedStyle: no_empty_lines (default)

# bad
module Foo

  def bar
    # ...
  end

end

# good
module Foo
  def bar
    # ...
  end
end
```

```ruby
# EnforcedStyle: empty_lines

# bad
module Foo
  def bar
    # ...
  end
end

# good
module Foo

  def bar
    # ...
  end

end
```

```ruby
# EnforcedStyle: empty_lines_except_namespace

# good
module Foo
  module Bar

    # ...

  end
end
```

```ruby
# EnforcedStyle: empty_lines_special

# good
module Foo

  def bar; end

end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ModuleNode],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("no_empty_lines"),
            allowed: &[
                "empty_lines",
                "empty_lines_except_namespace",
                "empty_lines_special",
                "no_empty_lines",
            ],
            doc: "The blank-line convention required around a module body.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "empty_lines" => Style::EmptyLines,
            "empty_lines_except_namespace" => Style::EmptyLinesExceptNamespace,
            "empty_lines_special" => Style::EmptyLinesSpecial,
            _ => Style::NoEmptyLines,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let module = node.as_module_node().expect("kind matched");
        let body = module.body();

        // RuboCop's `valid_body_style?`: an empty body is never flagged
        // unless the style is `no_empty_lines` (which forbids a blank line
        // even inside an otherwise-empty body).
        if body.is_none() && self.style != Style::NoEmptyLines {
            return;
        }

        let span = node.span();
        if ctx.is_single_line(span) {
            return;
        }

        let first_line = ctx.line_col(span.start).line;
        let last_line = ctx.last_line(span);

        match self.style {
            Style::EmptyLinesExceptNamespace => {
                let Some(body) = body else { return };
                let shape = shape_of(body);
                let want = if is_namespace(&shape, true) { Want::NoEmpty } else { Want::Empty };
                check_both(ctx, &Self::META, "module", want, first_line, last_line);
            }
            Style::EmptyLinesSpecial => {
                let Some(body) = body else { return };
                let shape = shape_of(body);
                if is_namespace(&shape, true) {
                    check_both(ctx, &Self::META, "module", Want::NoEmpty, first_line, last_line);
                } else {
                    if first_child_requires_empty_line(&shape) {
                        check_beginning(ctx, &Self::META, "module", Want::Empty, first_line);
                    } else {
                        check_beginning(ctx, &Self::META, "module", Want::NoEmpty, first_line);
                        check_deferred_empty_line(ctx, &Self::META, &shape);
                    }
                    check_ending(ctx, &Self::META, "module", Want::Empty, last_line);
                }
            }
            Style::NoEmptyLines => {
                check_both(ctx, &Self::META, "module", Want::NoEmpty, first_line, last_line);
            }
            Style::EmptyLines => {
                check_both(ctx, &Self::META, "module", Want::Empty, first_line, last_line);
            }
        }
    }
}
