//! `Rails/ActiveSupportOnLoad`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/active_support_on_load.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const LOAD_HOOKS: &[(&str, &str)] = &[
    ("ActionCable", "action_cable"),
    ("ActionCable::Channel::Base", "action_cable_channel"),
    ("ActionCable::Connection::Base", "action_cable_connection"),
    ("ActionCable::Connection::TestCase", "action_cable_connection_test_case"),
    ("ActionController::API", "action_controller"),
    ("ActionController::Base", "action_controller"),
    ("ActionController::TestCase", "action_controller_test_case"),
    ("ActionDispatch::IntegrationTest", "action_dispatch_integration_test"),
    ("ActionDispatch::Request", "action_dispatch_request"),
    ("ActionDispatch::Response", "action_dispatch_response"),
    ("ActionDispatch::SystemTestCase", "action_dispatch_system_test_case"),
    ("ActionMailbox::Base", "action_mailbox"),
    ("ActionMailbox::InboundEmail", "action_mailbox_inbound_email"),
    ("ActionMailbox::Record", "action_mailbox_record"),
    ("ActionMailbox::TestCase", "action_mailbox_test_case"),
    ("ActionMailer::Base", "action_mailer"),
    ("ActionMailer::TestCase", "action_mailer_test_case"),
    ("ActionText::Content", "action_text_content"),
    ("ActionText::Record", "action_text_record"),
    ("ActionText::RichText", "action_text_rich_text"),
    ("ActionView::Base", "action_view"),
    ("ActionView::TestCase", "action_view_test_case"),
    ("ActiveJob::Base", "active_job"),
    ("ActiveJob::TestCase", "active_job_test_case"),
    ("ActiveRecord::Base", "active_record"),
    ("ActiveStorage::Attachment", "active_storage_attachment"),
    ("ActiveStorage::Blob", "active_storage_blob"),
    ("ActiveStorage::Record", "active_storage_record"),
    ("ActiveStorage::VariantRecord", "active_storage_variant_record"),
    ("ActiveSupport::TestCase", "active_support_test_case"),
];

const RAILS_5_2_LOAD_HOOKS: &[(&str, &str)] =
    &[("ActiveRecord::ConnectionAdapters::SQLite3Adapter", "active_record_sqlite3adapter")];

const RAILS_7_1_LOAD_HOOKS: &[(&str, &str)] = &[
    ("ActiveRecord::TestFixtures", "active_record_fixtures"),
    ("ActiveModel::Model", "active_model"),
    ("ActionText::EncryptedRichText", "action_text_encrypted_rich_text"),
    ("ActiveRecord::ConnectionAdapters::PostgreSQLAdapter", "active_record_postgresqladapter"),
    ("ActiveRecord::ConnectionAdapters::Mysql2Adapter", "active_record_mysql2adapter"),
    ("ActiveRecord::ConnectionAdapters::TrilogyAdapter", "active_record_trilogyadapter"),
];

fn lookup(table: &[(&str, &'static str)], name: &str) -> Option<&'static str> {
    table.iter().find(|&&(k, _)| k == name).map(|&(_, v)| v)
}

/// Use `ActiveSupport.on_load(...)` to patch Rails framework classes.
#[derive(Debug, Clone)]
pub struct ActiveSupportOnLoad {
    rails_version: f64,
}

impl Rule for ActiveSupportOnLoad {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ActiveSupportOnLoad",
        department: Department::Rails,
        summary: "Use `ActiveSupport.on_load(...)` to patch Rails framework classes.",
        explanation: "Checks for Rails framework classes that are patched directly instead of \
                      using Active Support load hooks. Direct patching forcibly loads the \
                      framework referenced, using hooks defers loading until it's actually \
                      needed.\n\nThe autocorrection is unsafe: while using lazy load hooks is \
                      recommended, it changes the order in which is code is loaded and may \
                      reveal load order dependency bugs.\n\n```ruby\n# bad\n\
                      ActiveRecord::Base.include(MyClass)\n\n# good\n\
                      ActiveSupport.on_load(:active_record) { include MyClass }\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { rails_version: options.target_rails_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let method = call.name();
        let method = method.as_slice();
        if !matches!(method, b"prepend" | b"include" | b"extend") || call.is_safe_navigation() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        // `arguments` is the first child after the selector: the first
        // argument, or a `&blk` argument when that is all there is.
        let first = call
            .arguments()
            .and_then(|a| a.arguments().iter().next())
            .or_else(|| call.block().filter(|b| b.as_block_argument_node().is_some()));
        let Some(first) = first else { return };
        let Some(hook) = self.hook_for_const(const_name(&receiver).as_deref()) else { return };
        let method = String::from_utf8_lossy(method);
        let argument = String::from_utf8_lossy(ctx.text(first.span())).into_owned();
        let span = call_span_excluding_block(&call);
        let current = String::from_utf8_lossy(ctx.text(span)).into_owned();
        let preferred = format!("ActiveSupport.on_load(:{hook}) {{ {method} {argument} }}");
        let message = format!("Use `{preferred}` instead of `{current}`.");
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, preferred.into_bytes())],
            },
        );
    }
}

impl ActiveSupportOnLoad {
    fn hook_for_const(&self, name: Option<&str>) -> Option<&'static str> {
        let name = name?;
        let mut hook = lookup(LOAD_HOOKS, name);
        if hook.is_none() && self.rails_version >= 5.2 {
            hook = lookup(RAILS_5_2_LOAD_HOOKS, name);
        }
        if hook.is_none() && self.rails_version >= 7.1 {
            hook = lookup(RAILS_7_1_LOAD_HOOKS, name);
        }
        hook
    }
}
