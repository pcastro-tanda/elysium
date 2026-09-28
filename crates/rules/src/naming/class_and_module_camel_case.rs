//! `Naming/ClassAndModuleCamelCase`, ported from RuboCop's
//! `lib/rubocop/cop/naming/class_and_module_camel_case.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use CamelCase for classes and modules.";

/// Use CamelCase for classes and modules.
#[derive(Debug, Clone)]
pub struct ClassAndModuleCamelCase {
    allowed_names: Vec<String>,
}

impl ClassAndModuleCamelCase {
    fn check(&self, constant_path: &Node<'_>, ctx: &mut Context<'_>) {
        let span = constant_path.span();
        let source = String::from_utf8_lossy(ctx.text(span)).into_owned();
        if !source.contains('_') {
            return;
        }
        let mut stripped = source.clone();
        for allowed in &self.allowed_names {
            if allowed.is_empty() {
                continue;
            }
            stripped = stripped.replace(allowed.as_str(), "");
        }
        if !stripped.contains('_') {
            return;
        }
        ctx.report(&Self::META, span, MSG);
    }
}

impl Rule for ClassAndModuleCamelCase {
    const META: RuleMeta = RuleMeta {
        name: "Naming/ClassAndModuleCamelCase",
        department: Department::Naming,
        summary: "Use CamelCase for classes and modules.",
        explanation: "Checks for class and module names with \
an underscore in them.\n\n`AllowedNames` config takes an array of permitted names. \
Its default value is `['module_parent']`. These names can be full class/module \
names or part of the name. eg. Adding `my_class` to the `AllowedNames` config \
will allow names like `my_class`, `my_class::User`, `App::my_class`, \
`App::my_class::User`, etc.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::ModuleNode],
        config: &[ConfigOption {
            name: "AllowedNames",
            default: ConfigDefault::StrList(&["module_parent"]),
            allowed: &[],
            doc: "Permitted class/module names (full or partial).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_names: options.str_list("AllowedNames") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(class) = node.as_class_node() {
            self.check(&class.constant_path(), ctx);
        } else if let Some(module) = node.as_module_node() {
            self.check(&module.constant_path(), ctx);
        }
    }
}
