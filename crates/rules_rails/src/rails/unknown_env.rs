//! `Rails/UnknownEnv`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/unknown_env.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// `Environments` of `supports_local?`: `target_rails_version >= 7.1`.
const LOCAL_RAILS_VERSION: f64 = 7.1;

/// Use correct environment name.
#[derive(Debug, Clone)]
pub struct UnknownEnv {
    environments: Vec<String>,
    supports_local: bool,
}

impl Rule for UnknownEnv {
    const META: RuleMeta = RuleMeta {
        name: "Rails/UnknownEnv",
        department: Department::Rails,
        summary: "Use correct environment name.",
        explanation: "Checks that environments called with `Rails.env` predicates exist.\nBy \
                      default the cop allows three environments which Rails ships with: \
                      `development`, `test`, and `production`. More can be added to the \
                      `Environments` config parameter.\n\n```ruby\n# bad\n\
                      Rails.env.proudction?\nRails.env == 'proudction'\nRails.env != \
                      'proudction'\n\n# good\nRails.env.production?\nRails.env == \
                      'production'\nRails.env != 'production'\n```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::CaseNode],
        config: &[ConfigOption {
            name: "Environments",
            default: ConfigDefault::StrList(&["development", "test", "production"]),
            allowed: &[],
            doc: "Environment names that are considered known.",
        }],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            environments: options.str_list("Environments"),
            supports_local: options.target_rails_version() >= LOCAL_RAILS_VERSION,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(call) = node.as_call_node() {
            self.on_send(&call, ctx);
        } else if let Some(case) = node.as_case_node() {
            let Some(predicate) = case.predicate() else { return };
            if !is_rails_env(&predicate) {
                return;
            }
            for when in &case.conditions() {
                let Some(when) = when.as_when_node() else { continue };
                for condition in &when.conditions() {
                    let Some(string) = condition.as_string_node() else { continue };
                    let value = string.unescaped();
                    if self.unknown_env_name(value) {
                        let msg = self.message(&String::from_utf8_lossy(value));
                        ctx.report(&Self::META, condition.span(), msg);
                    }
                }
            }
        }
    }
}

impl UnknownEnv {
    fn on_send(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        let receiver = call.receiver();
        let arguments: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        let no_block = call.block().is_none();

        // unknown_environment_predicate?
        if let Some(receiver) = &receiver {
            if arguments.is_empty() && is_rails_env(receiver) && self.unknown_env_predicate(name) {
                if let Some(selector) = call.message_loc() {
                    let msg = self.message(&String::from_utf8_lossy(name));
                    ctx.report(&Self::META, selector.span(), msg);
                }
            }
        }

        // unknown_environment_equal?
        if !matches!(name, b"==" | b"===" | b"!=") || !no_block {
            return;
        }
        let (Some(receiver), [argument]) = (&receiver, arguments.as_slice()) else { return };
        let string = if is_rails_env(receiver) {
            argument
        } else if is_rails_env(argument) {
            receiver
        } else {
            return;
        };
        let Some(str_node) = string.as_string_node() else { return };
        let value = str_node.unescaped();
        if self.unknown_env_name(value) {
            let msg = self.message(&String::from_utf8_lossy(value));
            ctx.report(&Self::META, string.span(), msg);
        }
    }

    fn unknown_env_predicate(&self, name: &[u8]) -> bool {
        let Some(base) = name.strip_suffix(b"?") else { return false };
        let known = self.environments.iter().any(|e| e.as_bytes() == base)
            || (self.supports_local && base == b"local");
        !known
    }

    fn unknown_env_name(&self, name: &[u8]) -> bool {
        !self.environments.iter().any(|e| e.as_bytes() == name)
    }

    fn message(&self, name: &str) -> String {
        let name = name.strip_suffix('?').unwrap_or(name);
        let similar = spell_correct(name, &self.environments);
        if similar.is_empty() {
            format!("Unknown environment `{name}`.")
        } else {
            format!("Unknown environment `{name}`. Did you mean `{}`?", similar.join(", "))
        }
    }
}

/// `{(const nil? :Rails) (const (cbase) :Rails)}` `:env` receiver check
/// (`rails_env?` applied to the `Rails.env` send itself).
fn is_rails_env(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.is_safe_navigation()
        || call.name().as_slice() != b"env"
        || call.arguments().is_some()
        || call.block().is_some()
    {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    if let Some(constant) = receiver.as_constant_read_node() {
        return constant.name().as_slice() == b"Rails";
    }
    if let Some(path) = receiver.as_constant_path_node() {
        return path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"Rails");
    }
    false
}

/// `DidYouMean::SpellChecker#correct`.
fn spell_correct(input: &str, dictionary: &[String]) -> Vec<String> {
    let normalized: Vec<char> = input.to_lowercase().chars().collect();
    let threshold = if normalized.len() > 3 { 0.834 } else { 0.77 };
    let mut words: Vec<(&String, f64)> = dictionary
        .iter()
        .filter(|word| {
            let n: Vec<char> = word.to_lowercase().chars().collect();
            jaro_winkler(&n, &normalized) >= threshold
        })
        .filter(|word| word.as_str() != input)
        .map(|word| {
            let raw: Vec<char> = word.chars().collect();
            (word, jaro_winkler(&raw, &normalized))
        })
        .collect();
    // `sort_by!` then `reverse!`: ascending stable sort, reversed.
    words.sort_by(|a, b| a.1.total_cmp(&b.1));
    words.reverse();

    let max = normalized.len().div_ceil(4);
    let lower = |w: &str| w.to_lowercase().chars().collect::<Vec<char>>();
    let corrections: Vec<String> = words
        .iter()
        .filter(|(w, _)| levenshtein(&lower(w), &normalized) <= max)
        .map(|(w, _)| (*w).clone())
        .collect();
    if !corrections.is_empty() {
        return corrections;
    }
    words
        .iter()
        .filter(|(w, _)| {
            let w = lower(w);
            let length = normalized.len().min(w.len());
            levenshtein(&w, &normalized) < length
        })
        .take(1)
        .map(|(w, _)| (*w).clone())
        .collect()
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn jaro(s1: &[char], s2: &[char]) -> f64 {
    let (s1, s2) = if s1.len() > s2.len() { (s2, s1) } else { (s1, s2) };
    let (len1, len2) = (s1.len(), s2.len());
    let range = (len2 / 2).saturating_sub(1);
    let mut flags1 = vec![false; len1];
    let mut flags2 = vec![false; len2];
    let mut m = 0.0_f64;
    for (i, &c1) in s1.iter().enumerate() {
        let last = i + range;
        let mut j = i.saturating_sub(range);
        while j <= last {
            if j < len2 && !flags2[j] && c1 == s2[j] {
                flags2[j] = true;
                flags1[i] = true;
                m += 1.0;
                break;
            }
            j += 1;
        }
    }
    let mut k = 0;
    let mut t = 0.0_f64;
    for (i, &c1) in s1.iter().enumerate() {
        if flags1[i] {
            let mut j = k;
            while j < len2 {
                if flags2[j] {
                    k = j + 1;
                    break;
                }
                j += 1;
            }
            if s2.get(j) != Some(&c1) {
                t += 1.0;
            }
        }
    }
    t = (t / 2.0).floor();
    if m == 0.0 {
        0.0
    } else {
        (m / small(len1) + m / small(len2) + (m - t) / m) / 3.0
    }
}

fn jaro_winkler(s1: &[char], s2: &[char]) -> f64 {
    let distance = jaro(s1, s2);
    if distance > 0.7 {
        let mut prefix = 0.0;
        for (i, c) in s1.iter().enumerate() {
            if i < 4 && s2.get(i) == Some(c) {
                prefix += 1.0;
            } else {
                break;
            }
        }
        distance + prefix * 0.1 * (1.0 - distance)
    } else {
        distance
    }
}

/// Word lengths are tiny; the conversion is exact.
fn small(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}
