//! `Sorbet/StructPropName`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/struct_prop_name.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use std::sync::LazyLock;

use regex::Regex;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG_FORBIDDEN_SUFFIX: &str = "is forbidden, use another property name instead.";

/// `ConfigurableNaming::FORMATS[:snake_case]` (`[[:lower:]]` is Unicode-aware in Ruby).
static SNAKE_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@{0,2}[\d\p{Lowercase}_]+[!?=]?$").expect("valid regex"));

/// `ConfigurableNaming::FORMATS[:camelCase]`.
static CAMEL_CASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^@{0,2}(?:_|_?\p{Lowercase}[\d\p{Lowercase}\p{Uppercase}]*)[!?=]?$")
        .expect("valid regex")
});

/// Checks that `T::Struct` property names use the configured naming style.
#[derive(Debug, Clone)]
pub struct StructPropName {
    camel_case: bool,
    allowed_patterns: Vec<Regex>,
    forbidden_identifiers: Vec<String>,
    forbidden_patterns: Vec<Regex>,
    /// Enclosing `class`/`module`/`sclass`/`def` scopes, innermost last; the
    /// flag is whether the scope is a class inheriting from a `T::Struct`.
    scopes: Vec<bool>,
}

impl Rule for StructPropName {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/StructPropName",
        department: Department::Sorbet,
        summary: "Checks that T::Struct property names use the configured naming style.",
        explanation: "Checks that `T::Struct` property names use the configured style. The \
                      supported styles and name filters match `Naming/MethodName`.\n\n\
                      ```ruby\n# bad\nclass User < T::Struct\n  const :firstName, String\n  \
                      prop :lastName, String\nend\n\n# good\nclass User < T::Struct\n  \
                      const :first_name, String\n  prop :last_name, String\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
            NodeKind::SingletonClassNode,
            NodeKind::DefNode,
            NodeKind::CallNode,
        ],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("snake_case"),
                allowed: &["snake_case", "camelCase"],
                doc: "Naming style property names must follow.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Regexps; a property name matching one is never checked.",
            },
            ConfigOption {
                name: "ForbiddenIdentifiers",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Property names that are always flagged.",
            },
            ConfigOption {
                name: "ForbiddenPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Regexps; a property name matching one is always flagged.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let compile =
            |key: &str| options.str_list(key).iter().filter_map(|p| Regex::new(p).ok()).collect();
        Ok(Self {
            camel_case: options.style("EnforcedStyle")? == "camelCase",
            allowed_patterns: compile("AllowedPatterns"),
            forbidden_identifiers: options.str_list("ForbiddenIdentifiers"),
            forbidden_patterns: compile("ForbiddenPatterns"),
            scopes: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(class) = node.as_class_node() {
            self.scopes.push(class.superclass().is_some_and(|sup| t_struct(&sup)));
            return;
        }
        if matches!(
            node.kind(),
            NodeKind::ModuleNode | NodeKind::SingletonClassNode | NodeKind::DefNode
        ) {
            self.scopes.push(false);
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || !matches!(call.name().as_slice(), b"const" | b"prop") {
            return;
        }
        // `receiver.nil? || receiver.self_type?`
        if call.receiver().is_some_and(|r| r.as_self_node().is_none()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(name_node) = arguments.arguments().iter().next() else { return };
        let Some(symbol) = name_node.as_symbol_node() else { return };
        if self.scopes.last() != Some(&true) {
            return;
        }
        let name = String::from_utf8_lossy(symbol.unescaped());
        if self.allowed_patterns.iter().any(|p| p.is_match(&name)) {
            return;
        }
        let span = name_node.span();
        let stripped: String = name.chars().filter(|c| !matches!(c, '@' | '$')).collect();
        let forbidden = (!self.forbidden_identifiers.is_empty()
            && self.forbidden_identifiers.contains(&stripped))
            || self.forbidden_patterns.iter().any(|p| p.is_match(&name));
        if forbidden {
            ctx.report(&Self::META, span, format!("`{name}` {MSG_FORBIDDEN_SUFFIX}"));
        } else {
            let format = if self.camel_case { &CAMEL_CASE } else { &SNAKE_CASE };
            if !format.is_match(&name) {
                let style = if self.camel_case { "camelCase" } else { "snake_case" };
                ctx.report(&Self::META, span, format!("Use {style} for T::Struct property names."));
            }
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if matches!(
            node.kind(),
            NodeKind::ClassNode
                | NodeKind::ModuleNode
                | NodeKind::SingletonClassNode
                | NodeKind::DefNode
        ) {
            self.scopes.pop();
        }
    }
}

/// `(const (const {nil? cbase} :T) {:Struct :ImmutableStruct :InexactStruct})`
fn t_struct(node: &Node<'_>) -> bool {
    let Some(path) = node.as_constant_path_node() else { return false };
    let Some(parent) = path.parent() else { return false };
    let is_t = if let Some(read) = parent.as_constant_read_node() {
        read.name().as_slice() == b"T"
    } else if let Some(cbase_t) = parent.as_constant_path_node() {
        cbase_t.parent().is_none() && cbase_t.name().is_some_and(|n| n.as_slice() == b"T")
    } else {
        false
    };
    is_t && path.name().is_some_and(|name| {
        matches!(name.as_slice(), b"Struct" | b"ImmutableStruct" | b"InexactStruct")
    })
}
