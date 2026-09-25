#!/usr/bin/env bash
# Clones one corpus app at its pinned commit, runs real RuboCop and elysium
# over it with the same 49-cop `--only` list, times both, and diffs their
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

rules="$(grep -v '^#' "$script_dir/rules.txt" | grep -v '^[[:space:]]*$' | paste -sd, -)"

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

rubocop_json="$work/rubocop.json"
elysium_json="$work/elysium.json"

echo "== running rubocop over $app ==" >&2
SECONDS=0
(
  cd "$work"
  BUNDLE_GEMFILE="$app_dir/Gemfile" bundle exec rubocop \
    --cache false --only "$rules" --format json --out "$rubocop_json" .
) && rc_exit=0 || rc_exit=$?
rc_wall=$SECONDS
# RuboCop exits 1 when it finds offenses -- that's expected, not a failure.
# Only exit 2+ means RuboCop itself errored.
if [[ $rc_exit -gt 1 ]]; then
  echo "error: rubocop exited $rc_exit" >&2
  exit 2
fi

echo "== running elysium over $app ==" >&2
SECONDS=0
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
(
  cd "$work"
  # $work (the app checkout) commonly ships its own Gemfile.lock that also
  # happens to list the same plugin gems (e.g. rubocop-rails) at a version
  # unrelated to the one actually installed under $gem_path -- elysium
  # resolves gem versions against $BUNDLE_GEMFILE's own lockfile in
  # preference to that unrelated one when the variable is set, so export it
  # here too, not just for the `bundle exec` calls above.
  BUNDLE_GEMFILE="$app_dir/Gemfile" GEM_PATH="$gem_path" \
    "$elysium_bin" check --only "$rules" -f json . > "$elysium_json"
) && el_exit=0 || el_exit=$?
el_wall=$SECONDS
# Same convention as RuboCop: exit 1 = offenses found, not a failure.
if [[ $el_exit -gt 1 ]]; then
  echo "error: elysium exited $el_exit" >&2
  exit 2
fi

read_summary_field() {
  python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['summary'][sys.argv[2]])" "$1" "$2"
}

rc_files=$(read_summary_field "$rubocop_json" inspected_file_count)
rc_offenses=$(read_summary_field "$rubocop_json" offense_count)
el_files=$(read_summary_field "$elysium_json" inspected_file_count)
el_offenses=$(read_summary_field "$elysium_json" offense_count)

table="$(cat <<TABLE
### corpus: $app

| tool | wall s | files | offenses |
| --- | ---: | ---: | ---: |
| rubocop | $rc_wall | $rc_files | $rc_offenses |
| elysium | $el_wall | $el_files | $el_offenses |
TABLE
)"

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  { echo "$table"; echo; } >> "$GITHUB_STEP_SUMMARY"
else
  echo
  echo "$table"
  echo
fi

echo "== comparing offenses ==" >&2
python3 "$script_dir/compare.py" "$rubocop_json" "$elysium_json"
