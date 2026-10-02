//! `Rails/SquishedSQLHeredocs`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/squished_sql_heredocs.rb` (with core's `Heredoc`
//! mixin).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::ext::is_heredoc;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::sync::OnceLock;

/// Checks SQL heredocs to use `.squish`.
#[derive(Debug, Clone)]
pub struct SquishedSQLHeredocs {
    /// Heredocs that are the receiver or a direct argument of a `squish`
    /// call (`node.parent.send_type? && node.parent.method?(:squish)`).
    squished: Vec<Span>,
}

/// `OPENING_DELIMITER` is the regexp `(<<[~-]?)[quote]?([^quote]+)[quote]?` (quote = single, double or backtick).
fn opening_delimiter() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(<<[~-]?)['"`]?([^'"`]+)['"`]?"#).expect("static regex is valid")
    })
}

/// `SQL_IDENTIFIER_MARKERS`: `/(".+?")|('.+?')|(\[.+?\])/`.
fn sql_identifier_markers() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(".+?")|('.+?')|(\[.+?\])"#).expect("static regex is valid"))
}

impl Rule for SquishedSQLHeredocs {
    const META: RuleMeta = RuleMeta {
        name: "Rails/SquishedSQLHeredocs",
        department: Department::Rails,
        summary: "Checks SQL heredocs to use `.squish`.",
        explanation: "Checks SQL heredocs to use `.squish`.\n\nSome SQL syntax (e.g. PostgreSQL \
                      comments and functions) requires newlines to be preserved in order to \
                      work, thus autocorrection for this cop is not safe.\n\n```ruby\n# bad\n\
                      <<-SQL\n  SELECT * FROM posts;\nSQL\n\n<<-SQL\n  SELECT * FROM posts\n    \
                      WHERE id = 1\nSQL\n\nexecute(<<~SQL, \"Post Load\")\n  SELECT * FROM \
                      posts\n    WHERE post_id = 1\nSQL\n\n# good\n<<-SQL.squish\n  SELECT * \
                      FROM posts;\nSQL\n\n<<~SQL.squish\n  SELECT * FROM table\n    WHERE id = \
                      1\nSQL\n\nexecute(<<~SQL.squish, \"Post Load\")\n  SELECT * FROM posts\n    \
                      WHERE post_id = 1\nSQL\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::XStringNode,
            NodeKind::InterpolatedXStringNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { squished: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(call) = node.as_call_node() {
            if call.name().as_slice() == b"squish" && !call.is_safe_navigation() {
                if let Some(receiver) = call.receiver() {
                    self.squished.push(receiver.span());
                }
                if let Some(arguments) = call.arguments() {
                    self.squished.extend(arguments.arguments().iter().map(|a| a.span()));
                }
            }
            return;
        }
        if !is_heredoc(node) {
            return;
        }
        let span = node.span();
        let source = String::from_utf8_lossy(ctx.text(opening_span(node)));
        // `delimiter_string(node) == SQL`.
        let Some(captures) = opening_delimiter().captures(&source) else { return };
        if &captures[2] != "SQL" || self.squished.contains(&span) {
            return;
        }
        if singleline_comments_present(node, ctx) {
            return;
        }
        let current = source.into_owned();
        ctx.report_with_fix(
            &Self::META,
            opening_span(node),
            format!("Use `{current}.squish` instead of `{current}`."),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::insert(opening_span(node).end, b".squish".to_vec())],
            },
        );
    }
}

/// The heredoc opening (`<<~SQL`), which is what whitequark's `source` of a
/// heredoc node covers.
fn opening_span(node: &Node<'_>) -> Span {
    use ruby_ast::LocationExt as _;
    let opening = node
        .as_string_node()
        .and_then(|n| n.opening_loc())
        .or_else(|| node.as_interpolated_string_node().and_then(|n| n.opening_loc()))
        .or_else(|| node.as_x_string_node().map(|n| n.opening_loc()))
        .or_else(|| node.as_interpolated_x_string_node().map(|n| n.opening_loc()));
    opening.map_or(node.span(), |loc| loc.span())
}

/// `singleline_comments_present?`: the heredoc's children (a plain heredoc
/// is a single `str` holding its value; an interpolated one a list of line
/// `str`s and `begin`s), joined with a literal `\n`, contain `--` outside
/// quoted identifiers.
fn singleline_comments_present(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let parts: Vec<String> = if let Some(string) = node.as_string_node() {
        vec![String::from_utf8_lossy(string.unescaped()).into_owned()]
    } else if let Some(string) = node.as_interpolated_string_node() {
        string.parts().iter().map(|p| part_source(&p, ctx)).collect()
    } else if let Some(string) = node.as_interpolated_x_string_node() {
        string.parts().iter().map(|p| part_source(&p, ctx)).collect()
    } else if let Some(string) = node.as_x_string_node() {
        vec![String::from_utf8_lossy(string.unescaped()).into_owned()]
    } else {
        return false;
    };
    let sql = parts.join("\\n");
    sql_identifier_markers().replace_all(&sql, "").contains("--")
}

fn part_source(part: &Node<'_>, ctx: &Context<'_>) -> String {
    String::from_utf8_lossy(ctx.text(part.span())).into_owned()
}
