# Phase 3 rule selection

Ranked by real-world relevance: RuboCop 1.82.1 default enablement, the
resolved configuration of discourse/forem/mastodon, appearance in GitLab's
and forem's `.rubocop_todo` files (which means the cop fires on real code),
and presence in widely used shared configs. Only cops that need no scope,
variable, or constant tracking are eligible; the excluded borderline cases
move to Phase 4.

Autocorrect column follows RuboCop's `AutoCorrect`/`SafeAutoCorrect` in
`config/default.yml`.

| # | Cop | Score/6 | Autocorrect | What it needs |
|---|-----|---------|-------------|---------------|
| 1 | Layout/LineLength | 6 | safe | line text; heredoc/comment/URI exceptions |
| 2 | Layout/TrailingWhitespace | 6 | safe | line text |
| 3 | Style/FrozenStringLiteralComment | 6 | safe | magic comments |
| 4 | Style/StringLiterals | 6 | safe | `StringNode` delimiters and content |
| 5 | Layout/TrailingEmptyLines | 5 | safe | EOF bytes |
| 6 | Layout/IndentationWidth | 5 | safe | body statement columns vs parent |
| 7 | Layout/EmptyLines | 5 | safe | blank line runs |
| 8 | Layout/IndentationConsistency | 5 | safe | sibling statement columns |
| 9 | Style/Documentation | 5 | none | comment before `ClassNode`/`ModuleNode` |
| 10 | Style/GuardClause | 5 | none | `IfNode` ending in return/raise/next/break |
| 11 | Style/IfUnlessModifier | 5 | safe | single-statement `IfNode`, modifier line length |
| 12 | Style/HashSyntax | 5 | safe | `AssocNode` key form |
| 13 | Style/MutableConstant | 5 | unsafe | `ConstantWriteNode` RHS literal kind |
| 14 | Style/TrailingCommaInArguments | 5 | safe | multiline `CallNode` args, trailing comma |
| 15 | Style/TrailingCommaInHashLiteral | 5 | safe | multiline `HashNode`, trailing comma |
| 16 | Style/TrailingCommaInArrayLiteral | 5 | safe | multiline `ArrayNode`, trailing comma |
| 17 | Layout/EmptyLineBetweenDefs | 4 | safe | blank lines between sibling `DefNode`s |
| 18 | Layout/EmptyLinesAroundClassBody | 4 | safe | blank lines at `ClassNode` body edges |
| 19 | Layout/SpaceAroundOperators | 4 | safe | whitespace around operator tokens |
| 20 | Layout/SpaceInsideHashLiteralBraces | 4 | safe | whitespace inside `HashNode` braces |
| 21 | Style/SymbolProc | 4 | safe | `BlockNode` body is single call on block param |
| 22 | Layout/SpaceInsideArrayLiteralBrackets | 4 | safe | whitespace inside `ArrayNode` brackets |
| 23 | Layout/SpaceInsideBlockBraces | 4 | safe | whitespace inside `BlockNode` braces |
| 24 | Layout/ExtraSpacing | 4 | safe | whitespace runs outside alignment |
| 25 | Layout/HashAlignment | 4 | safe | `AssocNode` key/value columns |
| 26 | Style/WordArray | 4 | safe | `ArrayNode` of plain `StringNode`s |
| 27 | Style/NumericLiterals | 4 | safe | integer digit grouping |
| 28 | Style/RedundantParentheses | 4 | safe | `ParenthesesNode` around single safe child |
| 29 | Style/RedundantCondition | 4 | safe | `IfNode` branch equals condition |
| 30 | Layout/FirstHashElementIndentation | 4 | safe | first `AssocNode` column |
| 31 | Layout/FirstArgumentIndentation | 4 | safe | first argument column |
| 32 | Layout/ArgumentAlignment | 4 | safe | argument columns |
| 33 | Style/RedundantReturn | 3 | safe | trailing `ReturnNode` in `DefNode` |
| 34 | Style/SoleNestedConditional | 4 | safe | `IfNode` whose sole statement is `IfNode` |
| 35 | Style/ClassAndModuleChildren | 4 | unsafe | `class A::B` vs nested |
| 36 | Style/EmptyElse | 3 | contextual | empty `ElseNode` |
| 37 | Style/AccessorGrouping | 3 | safe | adjacent `attr_*` calls |
| 38 | Style/RedundantRegexpEscape | 3 | safe | regexp literal content |
| 39 | Style/RedundantRegexpCharacterClass | 3 | safe | regexp literal content |
| 40 | Style/NumericLiteralPrefix | 3 | safe | integer prefix casing |
| 41 | Style/IfUnlessModifierOfIfUnless | 3 | none | nested modifier shape |
| 42 | Style/StringConcatenation | 3 | unsafe | `+` chain of `StringNode`s |
| 43 | Lint/RedundantCopDisableDirective | 4 | safe | directives vs reported offenses |
| 44 | Lint/Debugger | 4 | none | `CallNode` name against configured list |
| 45 | Lint/DuplicateHashKey | 3 | none | `AssocNode` key literal equality |
| 46 | Lint/DuplicateMethods | 3 | none | sibling `DefNode` names in one body |
| 47 | Lint/AmbiguousBlockAssociation | 3 | safe | paren-less `CallNode` with `BlockNode` |
| 48 | Lint/EmptyBlock | 3 | none | empty `BlockNode` |
| 49 | Lint/ElseLayout | 3 | safe | `ElseNode` body on `else` line |
| 50 | Lint/RedundantStringCoercion | 3 | safe | `to_s` inside interpolation |

Reserve (if a top-50 cop turns out to need semantics): Layout/SpaceAfterComma,
Layout/SpaceBeforeComma, Layout/CommentIndentation, Layout/LeadingCommentSpace,
Layout/DotPosition, Layout/SpaceBeforeBlockBraces, Style/Semicolon, Style/Not,
Style/AndOr, Style/NegatedIf, Style/RescueStandardError, Style/RedundantBegin,
Style/RedundantFreeze, Style/HashEachMethods, Lint/RaiseException,
Lint/RescueException, Lint/LiteralAsCondition, Lint/OrderedMagicComments.

## Excluded as semantic (Phase 4)

Style/RedundantSelf (local vs method disambiguation), Lint/UselessAssignment,
Lint/ShadowedException (exception hierarchy), Lint/UselessAccessModifier,
Lint/MissingSuper, Lint/ConstantResolution, Lint/ShadowingOuterLocalVariable,
Lint/NumberConversion, Lint/SelfAssignment, Style/OptionalBooleanParameter.

## Batching

Rules 1–16 are batch one: they exercise every engine capability the rest
need (line-based rules, comment-based rules, node rules, node rules with
config options, safe and unsafe fixes). The fix engine and the
`fix` subcommand land with this batch. Rules 17–50 follow in batches of
~12 by department.
