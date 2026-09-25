//! `Lint/ConstantResolution`, ported from RuboCop's
//! `lib/rubocop/cop/lint/constant_resolution.rb`.
//!
//! RuboCop's `on_const` fires for every whitequark `:const` node, including
//! the namespace segments nested inside a longer path (`User::Login` is
//! `(const (const nil :User) :Login)`); only the innermost, `nil`-namespace
//! segment ever matches `unqualified_const?`'s `(const nil? ...)` shape, so
//! exactly the leftmost written segment of any constant path can be
//! flagged. Prism's split representation reaches the same node for that
//! same segment directly: a bare reference is a [`NodeKind::ConstantReadNode`]
//! whether it stands alone (`Login`) or anchors a longer
//! [`NodeKind::ConstantPathNode`] (`User` in `User::Login`, visited as that
//! node's own `parent` field); a leading `::` anchor never produces a
//! `ConstantReadNode` at all (`ConstantPathNode.parent` is `None`), so a
//! fully qualified reference is never a candidate to begin with and no
//! `ConstantPathNode` handling is needed.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Fully qualify this constant to avoid possibly ambiguous resolution.";

/// `Lint::ConstantResolution`.
#[derive(Debug, Clone)]
pub struct ConstantResolution {
    only: Vec<String>,
    ignore: Vec<String>,
}

impl ConstantResolution {
    /// RuboCop's `const_name?`.
    fn name_selected(&self, name: &[u8]) -> bool {
        let name = String::from_utf8_lossy(name);
        (self.only.is_empty() || self.only.iter().any(|n| n == name.as_ref()))
            && !self.ignore.iter().any(|n| n == name.as_ref())
    }
}

impl Rule for ConstantResolution {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ConstantResolution",
        department: Department::Lint,
        summary: "Checks that constants are fully qualified with `::`.",
        explanation: "\
This is not enabled by default because it would mark a lot of offenses
unnecessarily.

Generally, gems should fully qualify all constants to avoid conflicts with
the code that uses the gem. Enable this cop without using `Only`/`Ignore`.

Large projects will over time end up with one or two constant names that
are problematic because of a conflict with a library or just internally
using the same name a namespace and a class. To avoid too many unnecessary
offenses, enable this cop with `Only: [The, Constant, Names, Causing, Issues]`.

NOTE: `Style/RedundantConstantBase` cop is disabled if this cop is enabled to
prevent conflicting rules. Because it respects user configurations that want
to enable this cop which is disabled by default.

```ruby
# By default checks every constant

# bad
User

# bad
User::Login

# good
::User

# good
::User::Login
```

With `Only: ['Login']` (restrict this cop to only being concerned about
certain constants):

```ruby
# bad
Login

# good
::Login

# good
User::Login
```

With `Ignore: ['Login']` (restrict this cop from being concerned about
certain constants):

```ruby
# bad
User

# good
::User::Login

# good
Login
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ConstantReadNode],
        config: &[
            ConfigOption {
                name: "Only",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Restrict this cop to only looking at certain names.",
            },
            ConfigOption {
                name: "Ignore",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Restrict this cop from looking at certain names.",
            },
        ],
        blind_spots: "\
`node.parent&.defined_module` is narrowed to `ctx.parent()` being a
`ClassNode`/`ModuleNode`: upstream's `defined_module0` pattern also matches a
`(casgn ... (send Class/Module :new ...))` constant assignment, but only
when the assigned name is itself written as a compound path
(`A::B = Class.new`); a simple `Foo = Class.new` never produces a
`ConstantReadNode` for `Foo` in Prism to begin with (its name is a bare
field, not a child node), so the only gap is that rare compound-path form,
left unhandled here (never reported in RuboCop's own spec either). Per
upstream's own pattern shape, a `class Foo < Bar` superclass reference is
exempted exactly like the class's own name -- both are direct children of
the same `ClassNode` -- which this port reproduces rather than special-casing
away.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { only: options.str_list("Only"), ignore: options.str_list("Ignore") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let c = node.as_constant_read_node().expect("kind matched");
        if !self.name_selected(c.name().as_slice()) {
            return;
        }
        if matches!(ctx.parent().map(|p| p.kind), Some(NodeKind::ClassNode | NodeKind::ModuleNode))
        {
            return;
        }
        ctx.report(&Self::META, node.span(), MSG);
    }
}
