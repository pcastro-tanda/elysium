//! `Rails/ShortI18n`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/short_i18n.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// Enforces that short forms of `I18n` methods are used: `t` instead of
/// `translate` and `l` instead of `localize`.
#[derive(Debug, Clone)]
pub struct ShortI18n {
    /// `EnforcedStyle: conservative`.
    conservative: bool,
}

impl Rule for ShortI18n {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ShortI18n",
        department: Department::Rails,
        summary: "Use the short form of the I18n methods: `t` instead of `translate` and `l` instead of `localize`.",
        explanation: "Enforces that short forms of `I18n` methods are used: `t` instead of \
                      `translate` and `l` instead of `localize`.\n\nThis cop has two different \
                      enforcement modes. When the EnforcedStyle is conservative (the default) \
                      then only `I18n.translate` and `I18n.localize` calls are added as \
                      offenses.\n\nWhen the EnforcedStyle is aggressive then all `translate` \
                      and `localize` calls without a receiver are added as offenses.\n\n```ruby\n\
                      # bad\nI18n.translate :key\nI18n.localize Time.now\n\n# good\nI18n.t \
                      :key\nI18n.l Time.now\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("conservative"),
            allowed: &["conservative", "aggressive"],
            doc: "`aggressive` also flags receiverless `translate` and `localize`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { conservative: options.style("EnforcedStyle")? == "conservative" })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let good = match call.name().as_slice() {
            b"translate" => "t",
            b"localize" => "l",
            _ => return,
        };
        if call.is_safe_navigation() {
            return;
        }
        let receiver = call.receiver();
        if self.conservative && receiver.is_none() {
            return;
        }
        if let Some(receiver) = &receiver {
            if !is_bare_or_toplevel_const(receiver)
                || const_name(receiver).as_deref() != Some("I18n")
            {
                return;
            }
        }
        let Some(selector) = call.message_loc() else { return };
        let range = selector.span();
        let bad = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        ctx.report_with_fix(
            &Self::META,
            range,
            format!("Use `{good}` instead of `{bad}`."),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, good.as_bytes().to_vec())],
            },
        );
    }
}
