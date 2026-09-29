# Default-cop parity inventory
Generated from RuboCop 1.82.1 `config/default.yml` (`Enabled: true` only; `pending` cops excluded) minus the rules registered in `docs/rules/`. Regenerate with the script in this file's git history / `tools/` once ported there.
**59 default-enabled cops missing of 393** (334 implemented). `Style/DoubleCopDisableDirective` is excluded: RuboCop 1.91.0, the corpus truth, removed it.
## By department
- Style: 1
- Layout: 49
- Lint: 1
- Naming: 0
- Metrics: 8
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
| Layout/AccessModifierIndentation | 104 | 41 | autocorrect |
| Layout/ArrayAlignment | 84 | 25 | autocorrect, config-options |
| Layout/AssignmentIndentation | 57 | 10 | autocorrect |
| Layout/BeginEndAlignment | 73 | 0 | autocorrect |
| Layout/BlockAlignment | 259 | 79 | autocorrect |
| Layout/BlockEndNewline | 83 | 22 | autocorrect |
| Layout/CaseIndentation | 219 | 50 | autocorrect, config-options |
| Layout/ClosingHeredocIndentation | 123 | 11 | autocorrect |
| Layout/ClosingParenthesisIndentation | 193 | 43 | autocorrect |
| Layout/ConditionPosition | 60 | 5 | autocorrect |
| Layout/DefEndAlignment | 73 | 4 | autocorrect |
| Layout/DotPosition | 139 | 39 | autocorrect |
| Layout/ElseAlignment | 156 | 53 | autocorrect |
| Layout/EmptyLineAfterGuardClause | 217 | 47 | autocorrect, metrics |
| Layout/EmptyLinesAroundAccessModifier | 241 | 48 | autocorrect |
| Layout/EmptyLinesAroundArguments | 82 | 22 | autocorrect |
| Layout/EmptyLinesAroundAttributeAccessor | 139 | 20 | autocorrect |
| Layout/EmptyLinesAroundBeginBody | 41 | 10 | autocorrect |
| Layout/EmptyLinesAroundBlockBody | 41 | 12 | autocorrect |
| Layout/EmptyLinesAroundExceptionHandlingKeywords | 139 | 13 | autocorrect |
| Layout/EmptyLinesAroundMethodBody | 64 | 14 | autocorrect |
| Layout/EmptyLinesAroundModuleBody | 59 | 18 | autocorrect |
| Layout/EndAlignment | 214 | 14 | autocorrect |
| Layout/FirstArrayElementIndentation | 189 | 53 | autocorrect |
| Layout/FirstParameterIndentation | 101 | 20 | autocorrect |
| Layout/LeadingCommentSpace | 203 | 27 | tokens/comments, autocorrect, file-level, metrics, config-options |
| Layout/MultilineArrayBraceLayout | 115 | 4 | autocorrect |
| Layout/MultilineBlockLayout | 164 | 32 | autocorrect |
| Layout/MultilineHashBraceLayout | 115 | 3 | autocorrect |
| Layout/MultilineMethodCallBraceLayout | 132 | 15 | autocorrect |
| Layout/MultilineMethodCallIndentation | 267 | 120 | autocorrect, config-options |
| Layout/MultilineMethodDefinitionBraceLayout | 128 | 3 | autocorrect |
| Layout/MultilineOperationIndentation | 130 | 60 | autocorrect, config-options |
| Layout/ParameterAlignment | 118 | 19 | autocorrect, config-options |
| Layout/SpaceAfterColon | 49 | 12 | autocorrect |
| Layout/SpaceAfterComma | 32 | 5 | autocorrect |
| Layout/SpaceAfterMethodName | 39 | 8 | autocorrect |
| Layout/SpaceAfterNot | 39 | 6 | autocorrect |
| Layout/SpaceAfterSemicolon | 39 | 7 | autocorrect |
| Layout/SpaceAroundBlockParameters | 162 | 42 | autocorrect |
| Layout/SpaceAroundMethodCallOperator | 98 | 17 | autocorrect |
| Layout/SpaceBeforeBlockBraces | 164 | 20 | autocorrect, config-options |
| Layout/SpaceBeforeComma | 29 | 6 | autocorrect |
| Layout/SpaceBeforeFirstArg | 73 | 12 | autocorrect |
| Layout/SpaceBeforeSemicolon | 24 | 8 | autocorrect |
| Layout/SpaceInLambdaLiteral | 78 | 15 | autocorrect |
| Layout/SpaceInsideArrayPercentLiteral | 46 | 9 | autocorrect |
| Layout/SpaceInsidePercentLiteralDelimiters | 94 | 15 | autocorrect |
| Layout/SpaceInsideRangeLiteral | 54 | 7 | autocorrect |
| Lint/Syntax | 49 | 7 | pure-ast |
| Metrics/AbcSize | 56 | 22 | config-options |
| Metrics/BlockLength | 88 | 36 | metrics |
| Metrics/ClassLength | 77 | 34 | metrics |
| Metrics/CyclomaticComplexity | 58 | 37 | metrics |
| Metrics/MethodLength | 80 | 31 | metrics |
| Metrics/ModuleLength | 62 | 21 | metrics |
| Metrics/ParameterLists | 147 | 16 | config-options |
| Metrics/PerceivedComplexity | 59 | 31 | metrics |
| Style/ConditionalAssignment | 670 | 0 | autocorrect, metrics, config-options |
