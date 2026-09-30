# Default-cop parity inventory
Generated from RuboCop 1.82.1 `config/default.yml` (`Enabled: true` only; `pending` cops excluded) minus the rules registered in `docs/rules/`. Regenerate with the script in this file's git history / `tools/` once ported there.
**1 default-enabled cops missing of 393** (392 implemented). `Style/DoubleCopDisableDirective` is excluded: RuboCop 1.91.0, the corpus truth, removed it.
## By department
- Style: 0
- Layout: 0
- Lint: 1
- Naming: 0
- Metrics: 0
- Bundler: 0
- Gemspec: 0
- Security: 0
- Migration: 0

## By required infrastructure
Coarse, from mixins/API usage in the cop source. `pure-ast` = only node callbacks + `add_offense`. `tokens/comments` = needs the token stream or comment list (we have directives only). `target-ruby` = branches on `TargetRubyVersion`. `metrics` = needs code-length/complexity utilities. `semantic` = uses `VariableForce`.
- autocorrect: 257
- config-options: 52
- file-level: 39
- metrics: 30
- tokens/comments: 26
- target-ruby: 25
- semantic: 5
- ?: 4
- pure-ast: 1

## Full list
| Cop | src lines | spec `it`s | needs |
|---|---|---|---|
| Lint/Syntax | 49 | 7 | pure-ast |
