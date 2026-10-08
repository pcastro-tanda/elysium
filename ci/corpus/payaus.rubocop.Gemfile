# Side Gemfile for judging extension cops on payaus (read-only checkout, e.g.
# /tmp/payaus-target) against the pinned gem releases rather than whatever
# payaus's own Gemfile.lock resolves. xtask looks for
# `<parent of app>/<app>.rubocop.Gemfile`:
#
#   cp ci/corpus/payaus.rubocop.Gemfile /tmp/payaus-target.rubocop.Gemfile
#   BUNDLE_GEMFILE=/tmp/payaus-target.rubocop.Gemfile RBENV_VERSION=3.4.2 bundle lock --local
source "https://rubygems.org"

gem "rubocop", "1.91.0"
gem "rubocop-ast", "1.50.0"
gem "rubocop-minitest", "0.40.0"
gem "rubocop-performance", "1.27.0"
gem "rubocop-rails", "2.38.0"
gem "rubocop-sorbet", "0.16.0"
gem "rubocop-thread_safety", "0.8.0"
