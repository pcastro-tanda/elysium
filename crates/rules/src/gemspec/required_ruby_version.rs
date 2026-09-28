//! `Gemspec/RequiredRubyVersion`, ported from RuboCop's
//! `lib/rubocop/cop/gemspec/required_ruby_version.rb`.
//!
//! # Whole-file search, not ancestor-based
//!
//! Upstream's `on_new_investigation` runs a `def_node_search` over the *entire* file
//! (`processed_source.ast`) for any `required_ruby_version=` send anywhere, and reports
//! [`MISSING_MSG`] globally when none is found (including when the file is empty, i.e.
//! `processed_source.ast` itself is `nil`). Rather than a dedicated second tree walk, this
//! port reuses the single walk the engine already performs for `CallNode`: `enter` flips
//! [`RequiredRubyVersion::found`] the first time it sees a matching send anywhere in the
//! file, and `file_end` reports [`MISSING_MSG`] iff it never did.
//!
//! # `defined_ruby_version` / `extract_ruby_version`
//!
//! Upstream's `def_node_matcher` accepts three shapes for the assigned value: a bare string
//! literal, a two-element array of string literals, or `Gem::Requirement.new(*strings)`. For
//! the latter two, `extract_ruby_version` picks the *first* string containing `>` or `=`
//! (RuboCop's `/[>=]/`) -- if none matches, the whole thing counts as "no defined version"
//! (`nil`), which never equals `target_ruby_version.to_s` and so always offends. For a bare
//! string, no such filtering happens: whatever the string's digits are (even none, e.g. `''`)
//! is used directly. [`defined_ruby_version`] mirrors this three-way shape as
//! [`DefinedVersion`], and [`extract_ruby_version`] mirrors the digit-extraction exactly:
//! `str_content.scan(/\d/).first(2).join('.')`, i.e. the first two ASCII digit characters in
//! the string, joined with `.` (so `'~> 3'` -> `"3"`, `'>= 3.3.0'` -> `"3.3"`).
//!
//! Note the composed message never actually interpolates the extracted `required_ruby_version`
//! value (only `%<target_ruby_version>s` is used in `NOT_EQUAL_MSG`), so this port only needs
//! the extracted value for the equality check, never for display.
//!
//! # `dynamic_version?`
//!
//! Upstream: `(node.send_type? && !node.receiver) || node.variable? || node.each_descendant(:send,
//! *VARIABLES).any?` where `VARIABLES = %i[ivar gvar cvar lvar]`. In Prism terms: the value node
//! itself is a receiverless `CallNode` (a bare method call, not `Foo.bar`), or is itself a
//! local/instance/class/global-variable read, or has any such call/variable-read anywhere among
//! its descendants (e.g. an array literal containing a variable, as in
//! `[lowest_version, highest_version]`). [`is_dynamic_version`] mirrors this literally.
use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::node::StringNode;
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};

/// RuboCop's `MISSING_MSG`.
const MISSING_MSG: &str = "`required_ruby_version` should be specified.";

/// Whether `node` (the value assigned to `required_ruby_version`) is something whose Ruby
/// version cannot be determined statically -- RuboCop's `dynamic_version?`. See the module
/// doc's "`dynamic_version?`" section.
fn is_dynamic_version(node: &Node<'_>) -> bool {
    if let Some(call) = node.as_call_node() {
        if call.receiver().is_none() {
            return true;
        }
    }
    if is_variable_read(node.kind()) {
        return true;
    }
    let mut dynamic = false;
    each_descendant(node, &mut |child| {
        if !dynamic && (child.as_call_node().is_some() || is_variable_read(child.kind())) {
            dynamic = true;
        }
    });
    dynamic
}

/// RuboCop's `RuboCop::AST::Node::VARIABLES` (`%i[ivar gvar cvar lvar]`).
fn is_variable_read(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableReadNode
            | NodeKind::InstanceVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::GlobalVariableReadNode
    )
}

/// The three shapes RuboCop's `defined_ruby_version` `def_node_matcher` accepts. See the module
/// doc's "`defined_ruby_version` / `extract_ruby_version`" section.
enum DefinedVersion<'pr> {
    /// A bare string literal, used as-is with no filtering.
    Single(StringNode<'pr>),
    /// A two-element array of string literals, or `Gem::Requirement.new(*strings)`: filtered
    /// down to the first element containing `>` or `=`.
    Many(Vec<StringNode<'pr>>),
}

/// Whether `call` is `Gem::Requirement.new(...)`: RuboCop's
/// `(send (const (const nil? :Gem) :Requirement) :new $str+)` receiver shape.
fn is_gem_requirement_new(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.name().as_slice() != b"new" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    const_name(&receiver).as_deref() == Some("Gem::Requirement")
}

/// RuboCop's `defined_ruby_version` `def_node_matcher`, applied to the value assigned to
/// `required_ruby_version`. Returns `None` if `node` matches none of the three accepted shapes.
fn defined_ruby_version<'pr>(node: &Node<'pr>) -> Option<DefinedVersion<'pr>> {
    if let Some(s) = node.as_string_node() {
        return Some(DefinedVersion::Single(s));
    }
    if let Some(array) = node.as_array_node() {
        let elements: Vec<Node<'pr>> = array.elements().iter().collect();
        if let [a, b] = elements.as_slice() {
            if let (Some(a), Some(b)) = (a.as_string_node(), b.as_string_node()) {
                return Some(DefinedVersion::Many(vec![a, b]));
            }
        }
        return None;
    }
    if is_gem_requirement_new(node) {
        let call = node.as_call_node().expect("checked by is_gem_requirement_new");
        let args = call.arguments()?;
        let mut strings = Vec::new();
        for arg in &args.arguments() {
            strings.push(arg.as_string_node()?);
        }
        if strings.is_empty() {
            return None;
        }
        return Some(DefinedVersion::Many(strings));
    }
    None
}

/// RuboCop's `extract_ruby_version`. See the module doc's "`defined_ruby_version` /
/// `extract_ruby_version`" section for the digit-extraction rule.
fn extract_ruby_version(defined: Option<DefinedVersion<'_>>) -> Option<String> {
    let candidate = match defined? {
        DefinedVersion::Single(s) => s,
        DefinedVersion::Many(candidates) => candidates
            .into_iter()
            .find(|s| s.unescaped().iter().any(|&b| b == b'>' || b == b'='))?,
    };
    let digits: String = candidate
        .unescaped()
        .iter()
        .filter(|b| b.is_ascii_digit())
        .take(2)
        .map(|&b| b as char)
        .collect();
    Some(digits.chars().enumerate().fold(String::new(), |mut acc, (i, c)| {
        if i > 0 {
            acc.push('.');
        }
        acc.push(c);
        acc
    }))
}

/// Checks that `required_ruby_version` of gemspec is specified and equal to `TargetRubyVersion`
/// of .rubocop.yml.
///
/// This ensures that RuboCop is using the same Ruby version as the gem.
///
/// # Examples
///
/// ```ruby
/// # When `TargetRubyVersion` of .rubocop.yml is `2.5`.
///
/// # bad
/// Gem::Specification.new do |spec|
///   # no `required_ruby_version` specified
/// end
///
/// # bad
/// Gem::Specification.new do |spec|
///   spec.required_ruby_version = '>= 2.4.0'
/// end
///
/// # good
/// Gem::Specification.new do |spec|
///   spec.required_ruby_version = '>= 2.5.0'
/// end
///
/// # accepted but not recommended
/// Gem::Specification.new do |spec|
///   spec.required_ruby_version = ['>= 2.5.0', '< 2.7.0']
/// end
/// ```
#[derive(Debug, Clone)]
pub struct RequiredRubyVersion {
    target_ruby_version: f32,
    /// Whether a `required_ruby_version=` send has been seen anywhere in the file so far (see
    /// the module doc's "Whole-file search" section).
    found: bool,
}

impl Rule for RequiredRubyVersion {
    const META: RuleMeta = RuleMeta {
        name: "Gemspec/RequiredRubyVersion",
        department: Department::Gemspec,
        summary: "Checks that `required_ruby_version` of gemspec is specified and equal to `TargetRubyVersion` of .rubocop.yml.",
        explanation: "\
Checks that `required_ruby_version` of gemspec is specified and equal to `TargetRubyVersion`
of .rubocop.yml.

This ensures that RuboCop is using the same Ruby version as the gem.

```ruby
# When `TargetRubyVersion` of .rubocop.yml is `2.5`.

# bad
Gem::Specification.new do |spec|
  # no `required_ruby_version` specified
end

# bad
Gem::Specification.new do |spec|
  spec.required_ruby_version = '>= 2.4.0'
end

# good
Gem::Specification.new do |spec|
  spec.required_ruby_version = '>= 2.5.0'
end

# accepted but not recommended
Gem::Specification.new do |spec|
  spec.required_ruby_version = ['>= 2.5.0', '< 2.7.0']
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version(), found: false })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.found = false;
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"required_ruby_version=" {
            return;
        }
        self.found = true;

        let Some(version_def) = call.arguments().and_then(|args| args.arguments().iter().next())
        else {
            return;
        };
        if is_dynamic_version(&version_def) {
            return;
        }
        let ruby_version = extract_ruby_version(defined_ruby_version(&version_def));
        let target = format_target_ruby_version(self.target_ruby_version);
        if ruby_version.as_deref() == Some(target.as_str()) {
            return;
        }
        let message = format!(
            "`required_ruby_version` and `TargetRubyVersion` ({target}, which may be specified in .rubocop.yml) should be equal."
        );
        ctx.report(&Self::META, version_def.span(), message);
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        if !self.found {
            ctx.report_global(&Self::META, MISSING_MSG);
        }
    }
}

/// Ruby's `Float#to_s` for the one-decimal-or-more values `TargetRubyVersion` takes (e.g. `3.4`
/// -> `"3.4"`, `3.0` -> `"3.0"`).
fn format_target_ruby_version(version: f32) -> String {
    format!("{version:.1}")
}
