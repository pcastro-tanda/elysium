//! `Rails/I18nLazyLookup`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/i18n_lazy_lookup.rb`.

use std::collections::HashMap;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

use crate::inflector::underscore;
use crate::parent_module::{self, ParentModule};

/// A lexical `class` ancestor.
#[derive(Debug, Clone)]
struct ClassScope {
    /// `class.identifier.source`.
    identifier: String,
    /// `class.parent_module_name`.
    parent_module: Option<String>,
}

/// Checks for places where I18n "lazy" lookup can be used.
#[derive(Debug, Clone)]
pub struct I18nLazyLookup {
    lazy: bool,
    classes: Vec<ClassScope>,
    modules: ParentModule,
    /// Receiverless `def` ancestors: whether each is public.
    defs: Vec<(bool, String)>,
    /// Start offset of a `def` -> whether it is public.
    visibility: HashMap<u32, bool>,
}

impl Rule for I18nLazyLookup {
    const META: RuleMeta = RuleMeta {
        name: "Rails/I18nLazyLookup",
        department: Department::Rails,
        summary: "Checks for places where I18n \"lazy\" lookup can be used.",
        explanation: "Checks for places where I18n \"lazy\" lookup can be used.\n\nThis cop has \
                      two different enforcement modes. When the EnforcedStyle is `lazy` (the \
                      default), explicit lookups are added as offenses.\n\nWhen the \
                      EnforcedStyle is `explicit` then lazy lookups are added as offenses.\n\n\
                      ```ruby\n# bad (lazy)\nclass BooksController < ApplicationController\n  \
                      def create\n    redirect_to books_url, notice: \
                      t('books.create.success')\n  end\nend\n\n# good (lazy)\nclass \
                      BooksController < ApplicationController\n  def create\n    redirect_to \
                      books_url, notice: t('.success')\n  end\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
            NodeKind::SingletonClassNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::LambdaNode,
            NodeKind::DefNode,
            NodeKind::StatementsNode,
            NodeKind::CallNode,
        ],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("lazy"),
            allowed: &["lazy", "explicit"],
            doc: "`lazy` flags explicit lookups, `explicit` flags lazy lookups.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            lazy: options.style("EnforcedStyle")? == "lazy",
            classes: Vec::new(),
            modules: ParentModule::default(),
            defs: Vec::new(),
            visibility: HashMap::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if parent_module::KINDS.contains(&node.kind()) {
            if let Some(class) = node.as_class_node() {
                let identifier =
                    String::from_utf8_lossy(ctx.text(class.constant_path().span())).into_owned();
                self.classes.push(ClassScope { identifier, parent_module: self.modules.name() });
            }
            self.modules.enter(node);
        }
        match node.kind() {
            NodeKind::StatementsNode => {
                let stmts = node.as_statements_node().expect("kind matched");
                let stmts: Vec<Node<'_>> = stmts.body().iter().collect();
                self.record_visibility(&stmts);
            }
            NodeKind::DefNode => {
                let def = node.as_def_node().expect("kind matched");
                if def.receiver().is_none() {
                    let public = self.visibility.get(&node.span().start).copied().unwrap_or(true);
                    let name = String::from_utf8_lossy(def.name().as_slice()).into_owned();
                    self.defs.push((public, name));
                }
            }
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                self.check_call(&call, ctx);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if parent_module::KINDS.contains(&node.kind()) {
            self.modules.leave();
            if node.kind() == NodeKind::ClassNode {
                self.classes.pop();
            }
        }
        if node.kind() == NodeKind::DefNode
            && node.as_def_node().is_some_and(|d| d.receiver().is_none())
        {
            self.defs.pop();
        }
    }
}

fn visibility_scope(name: &[u8]) -> bool {
    matches!(name, b"private" | b"protected" | b"public")
}

/// `(send nil? {:private :protected :public})`.
fn is_visibility_block(node: &Node<'_>) -> Option<Vec<u8>> {
    let call = node.as_call_node()?;
    (call.receiver().is_none()
        && visibility_scope(call.name().as_slice())
        && call.arguments().is_none_or(|a| a.arguments().is_empty()))
    .then(|| call.name().as_slice().to_vec())
}

/// `(send nil? {:private :protected :public} (sym method_name))`.
fn is_inline_on_method_name(node: &Node<'_>, method: &[u8]) -> Option<Vec<u8>> {
    let call = node.as_call_node()?;
    if call.receiver().is_some() || !visibility_scope(call.name().as_slice()) {
        return None;
    }
    let args: Vec<Node<'_>> = call.arguments()?.arguments().iter().collect();
    let [arg] = args.as_slice() else { return None };
    let sym = arg.as_symbol_node()?;
    (sym.unescaped() == method).then(|| call.name().as_slice().to_vec())
}

impl I18nLazyLookup {
    /// `VisibilityHelp#node_visibility` for every `def` among `stmts` (and
    /// `private def ...` inline forms).
    fn record_visibility(&mut self, stmts: &[Node<'_>]) {
        for (i, stmt) in stmts.iter().enumerate() {
            if let Some(def) = stmt.as_def_node() {
                let name = def.name();
                let visibility = stmts[i + 1..]
                    .iter()
                    .rev()
                    .find_map(|s| is_inline_on_method_name(s, name.as_slice()))
                    .or_else(|| stmts[..i].iter().rev().find_map(is_visibility_block));
                let public = visibility.is_none_or(|v| v == b"public");
                self.visibility.insert(stmt.span().start, public);
            } else if let Some(call) = stmt.as_call_node() {
                if call.receiver().is_none() && visibility_scope(call.name().as_slice()) {
                    if let Some(args) = call.arguments() {
                        let args: Vec<Node<'_>> = args.arguments().iter().collect();
                        if let [arg] = args.as_slice() {
                            if arg.as_def_node().is_some() {
                                self.visibility
                                    .insert(arg.span().start, call.name().as_slice() == b"public");
                            }
                        }
                    }
                }
            }
        }
    }

    fn check_call(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if call.receiver().is_some() || !matches!(call.name().as_slice(), b"translate" | b"t") {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(key_node) = arguments.arguments().iter().next() else { return };
        let key: String = if let Some(s) = key_node.as_string_node() {
            String::from_utf8_lossy(s.unescaped()).into_owned()
        } else if let Some(s) = key_node.as_symbol_node() {
            String::from_utf8_lossy(s.unescaped()).into_owned()
        } else {
            return;
        };
        let starts_with_dot = key.starts_with('.');
        if self.lazy == starts_with_dot {
            return;
        }
        let Some((path, action_name)) = self.controller_action() else { return };
        let last = key.rsplit('.').next().unwrap_or("");
        let scoped_key = format!("{path}.{action_name}.{last}");
        let span = key_node.span();
        let replacement = if self.lazy {
            if key != scoped_key {
                return;
            }
            format!("'.{last}'")
        } else {
            format!("'{scoped_key}'")
        };
        let message = if self.lazy {
            "Use lazy lookup for the text used in controllers."
        } else {
            "Use explicit lookup for the text used in controllers."
        };
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            },
        );
    }

    /// `controller_and_action`: the dotted controller path (sans action) and
    /// the action name, when inside a public `def` in a `*Controller` class.
    fn controller_action(&self) -> Option<(String, String)> {
        let (public, action) = self.defs.last()?;
        if !public {
            return None;
        }
        let class = self.classes.last()?;
        if !class.identifier.ends_with("Controller") {
            return None;
        }
        let module_name = class.parent_module.as_deref().unwrap_or_default();
        let path = if module_name == "Object" {
            class.identifier.clone()
        } else {
            format!("{module_name}::{}", class.identifier)
        };
        let path = path.strip_suffix("Controller").unwrap_or(&path);
        Some((underscore(path).replace('/', "."), action.clone()))
    }
}
