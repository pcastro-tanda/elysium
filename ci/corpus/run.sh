#!/usr/bin/env bash
# Clones one corpus app at its pinned commit, runs real RuboCop and elysium
# over it with `--only` in two passes -- `app`: the stable cops in rules.txt
# that the app's own config enables (enabled_cops.rb); `defaults`: every
# stable cop under RuboCop's default config -- times both, and diffs their
# offenses via compare.py. Used by .github/workflows/corpus.yml; runnable
# locally the same way.
#
# If $RUNNER_TEMP/corpus-<app> already holds a checkout whose HEAD is the
# pinned commit (e.g. restored by actions/cache, keyed on that commit), the
# clone is skipped and the existing checkout is reused as-is.
#
# Usage: ci/corpus/run.sh <app>          (app: discourse | mastodon | forem)
#
# Env:
#   ELYSIUM_BIN   path to the elysium binary to test
#                 (default: $REPO_ROOT/target/release/elysium)
#   RUNNER_TEMP   scratch directory for the app checkout
#                 (default: a fresh mktemp -d)
#   GITHUB_STEP_SUMMARY  when set, the results table is appended there
#                 instead of printed to stdout (set by GitHub Actions)
set -euo pipefail

app="${1:?usage: run.sh <app>}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
app_dir="$script_dir/$app"

if [[ ! -f "$app_dir/app.env" ]]; then
  echo "error: unknown app '$app' (no $app_dir/app.env)" >&2
  exit 2
fi

# shellcheck source=/dev/null
source "$app_dir/app.env"
: "${REPO:?$app_dir/app.env must set REPO}"
: "${COMMIT:?$app_dir/app.env must set COMMIT}"

elysium_bin="${ELYSIUM_BIN:-$repo_root/target/release/elysium}"
if [[ ! -x "$elysium_bin" ]]; then
  echo "error: elysium binary not found or not executable at $elysium_bin" >&2
  exit 2
fi

work_root="${RUNNER_TEMP:-$(mktemp -d)}"
work="$work_root/corpus-$app"

if git -C "$work" rev-parse --verify --quiet HEAD >/dev/null 2>&1 \
    && [[ "$(git -C "$work" rev-parse HEAD)" == "$COMMIT" ]]; then
  echo "== reusing cached checkout for $app @ $COMMIT ==" >&2
else
  rm -rf "$work"
  mkdir -p "$work"
  echo "== cloning $app @ $COMMIT ==" >&2
  git init --quiet "$work"
  git -C "$work" remote add origin "$REPO"
  git -C "$work" fetch --quiet --filter=blob:none --depth 1 origin "$COMMIT"
  git -C "$work" checkout --quiet "$COMMIT"
fi

# elysium's `inherit_gem`/plugin-default lookup walks `vendor/bundle/**` under
# the directory it lints (here, $work -- the app checkout) plus GEM_HOME,
# GEM_PATH, and BUNDLE_PATH from its environment. `bundle install`'s cache
# path is resolved by ruby/setup-ruby relative to $GITHUB_WORKSPACE (this
# repo checkout), not $work, and elysium has no other way to learn where
# that is -- so surface it explicitly via GEM_PATH, the same directories
# RubyGems itself would search inside this Gemfile's bundle context.
gem_path="$(
  cd "$work"
  BUNDLE_GEMFILE="$app_dir/Gemfile" bundle exec ruby -e 'print Gem.path.join(":")'
)"
echo "gem_path=$gem_path" >&2

read_summary_field() {
  python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['summary'][sys.argv[2]])" "$1" "$2"
}

# plugins_for <rules-csv>: the extension gems (one per line) whose departments
# appear in the rules, e.g. Rails/Pick -> rubocop-rails. Keep in sync with
# EXTENSION_GEMS in enabled_cops.rb and crates/xtask/src/conformance_rule.rs.
# The gem versions come from this app's Gemfile (rails 2.38.0, performance
# 1.27.0, minitest 0.40.0, sorbet 0.16.0, thread_safety 0.8.0).
plugins_for() {
  tr ',' '\n' <<<"$1" | cut -d/ -f1 | sort -u | while read -r dept; do
    case "$dept" in
      Rails) echo rubocop-rails ;;
      Performance) echo rubocop-performance ;;
      Minitest) echo rubocop-minitest ;;
      Sorbet) echo rubocop-sorbet ;;
      ThreadSafety) echo rubocop-thread_safety ;;
    esac
  done
}

# run_pass <pass> <rules-csv> [--defaults]
#
# Runs RuboCop and elysium over $work with `--only <rules-csv>`, writes
# $work/{rubocop,elysium}.<pass>.json and $work/compare.<pass>.txt, and
# returns compare.py's status (0 = exact match). With --defaults both tools
# ignore the app's config (`--force-default-config` / `--no-config`).
run_pass() {
  local pass="$1" rules="$2" defaults="${3:-}"
  local rc_flags=() el_flags=() plugins plugin
  plugins="$(plugins_for "$rules")"
  if [[ "$defaults" == --defaults ]]; then
    rc_flags=(--force-default-config)
    el_flags=(--no-config)
  fi
  # `--plugin` loads the department's gem (and its default.yml) whether or not
  # the app's config names it; with --force-default-config that is RuboCop's
  # defaults plus the gem's. elysium has no `--plugin`: it layers gem defaults
  # only for a config file's `plugins:`, so hand it a throwaway config that
  # lists them (and, in the app pass, inherits the app's .rubocop.yml).
  local el_config=""
  if [[ -n "$plugins" ]]; then
    for plugin in $plugins; do rc_flags+=(--plugin "$plugin"); done
    el_config="$(mktemp "${TMPDIR:-/tmp}/elysium-plugins.XXXXXX")"
    {
      if [[ "$defaults" != --defaults && -f "$work/.rubocop.yml" ]]; then
        echo "inherit_from: $work/.rubocop.yml"
      fi
      echo "plugins:"
      for plugin in $plugins; do echo "  - $plugin"; done
    } >"$el_config"
    el_flags=(--config "$el_config")
  fi
  local rubocop_json="$work/rubocop.$pass.json" elysium_json="$work/elysium.$pass.json"
  local rc_exit el_exit rc_wall el_wall

  echo "== [$pass] running rubocop over $app ($(tr ',' '\n' <<<"$rules" | wc -l | tr -d ' ') cops) ==" >&2
  SECONDS=0
  (
    cd "$work"
    BUNDLE_GEMFILE="$app_dir/Gemfile" bundle exec rubocop ${rc_flags[@]+"${rc_flags[@]}"} \
      --cache false --only "$rules" --format json --out "$rubocop_json" .
  ) && rc_exit=0 || rc_exit=$?
  rc_wall=$SECONDS
  # RuboCop exits 1 when it finds offenses -- that's expected, not a failure.
  # Only exit 2+ means RuboCop itself errored.
  if [[ $rc_exit -gt 1 ]]; then
    echo "error: rubocop exited $rc_exit" >&2
    exit 2
  fi

  echo "== [$pass] running elysium over $app ==" >&2
  SECONDS=0
  (
    cd "$work"
    # $work (the app checkout) commonly ships its own Gemfile.lock that also
    # happens to list the same plugin gems (e.g. rubocop-rails) at a version
    # unrelated to the one actually installed under $gem_path -- elysium
    # resolves gem versions against $BUNDLE_GEMFILE's own lockfile in
    # preference to that unrelated one when the variable is set, so export it
    # here too, not just for the `bundle exec` calls above.
    BUNDLE_GEMFILE="$app_dir/Gemfile" GEM_PATH="$gem_path" \
      "$elysium_bin" check ${el_flags[@]+"${el_flags[@]}"} --only "$rules" -f json . > "$elysium_json"
  ) && el_exit=0 || el_exit=$?
  el_wall=$SECONDS
  [[ -z "$el_config" ]] || rm -f "$el_config"
  # Same convention as RuboCop: exit 1 = offenses found, not a failure.
  if [[ $el_exit -gt 1 ]]; then
    echo "error: elysium exited $el_exit" >&2
    exit 2
  fi

  local table
  table="$(cat <<TABLE
### corpus: $app ($pass)

| tool | wall s | files | offenses |
| --- | ---: | ---: | ---: |
| rubocop | $rc_wall | $(read_summary_field "$rubocop_json" inspected_file_count) | $(read_summary_field "$rubocop_json" offense_count) |
| elysium | $el_wall | $(read_summary_field "$elysium_json" inspected_file_count) | $(read_summary_field "$elysium_json" offense_count) |
TABLE
)"
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    { echo "$table"; echo; } >> "$GITHUB_STEP_SUMMARY"
  else
    echo
    echo "$table"
    echo
  fi

  echo "== [$pass] comparing offenses ==" >&2
  python3 "$script_dir/compare.py" "$rubocop_json" "$elysium_json" | tee "$work/compare.$pass.txt"
}

all_rules="$(grep -v '^#' "$script_dir/rules.txt" | grep -v '^[[:space:]]*$' | paste -sd, -)"
# `--only` force-enables every listed cop, even ones the app's config turns
# off, so the `app` pass narrows the list to what the app really runs: its
# offense count is the app's real lint result for these cops.
app_rules="$(
  cd "$work"
  BUNDLE_GEMFILE="$app_dir/Gemfile" bundle exec ruby "$script_dir/enabled_cops.rb" "$script_dir/rules.txt"
)"

# Apps are typically lint-clean under their own config, so the `app` pass
# mostly proves elysium adds no false positives. The `defaults` pass runs
# every stable cop under RuboCop's default config, where the app has real
# offenses to find, so it also catches offenses elysium misses.
status=0
run_pass app "$app_rules" || status=1
run_pass defaults "$all_rules" --defaults || status=1
exit "$status"
