# Style/MethodCallWithArgsParentheses

Use parentheses for method calls with arguments.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

In the default style (require_parentheses), macro methods are allowed. Additional methods can be added to the `AllowedMethods` or `AllowedPatterns` list. These options are valid only in the default style. Macros can be included by either setting `IgnoreMacros` to false, adding specific macros to the `IncludedMacros` list, or using `IncludedMacroPatterns` for pattern-based matching.

In the alternative style (omit_parentheses), `AllowParenthesesInChaining`,
`AllowParenthesesInMultilineCall`, and `AllowParenthesesInCamelCaseMethod` allow parentheses in those specific cases.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `require_parentheses` | `require_parentheses`, `omit_parentheses` | Whether method calls with arguments require or omit parentheses. |
| IgnoreMacros | true |  | `require_parentheses` only: whether macro calls are exempt. |
| AllowedMethods | `[]` |  | `require_parentheses` only: method names always allowed without parens. |
| AllowedPatterns | `[]` |  | `require_parentheses` only: method name regex patterns always allowed. |
| IncludedMacros | `[]` |  | `require_parentheses` only: macro method names never exempted. |
| IncludedMacroPatterns | `[]` |  | `require_parentheses` only: macro method name regex patterns never exempted. |
| AllowParenthesesInMultilineCall | false |  | `omit_parentheses` only: whether a multi-line call may keep parentheses. |
| AllowParenthesesInChaining | false |  | `omit_parentheses` only: whether the last call in a chain may keep parentheses. |
| AllowParenthesesInCamelCaseMethod | false |  | `omit_parentheses` only: whether a capitalized method call may keep parentheses. |
| AllowParenthesesInStringInterpolation | false |  | `omit_parentheses` only: whether a call inside string interpolation may keep parentheses. |

## Blind spots

The upstream `OmitParentheses` reparse-verification safety net (`on_investigation_end`'s
`verified_by_reparse`, which re-parses a corrected copy to confirm the omission does not change
how the code parses) is not reproduced; this port relies solely on the structural predicates
(`legitimate_call_with_parentheses?` and friends) the fixtures exercise. `class_constructor?`
(`Class.new`/`Module.new`/`Struct.new`/`Data.define`, used by `in_macro_scope?`'s parent check) and
the `any_block`/`if`-excluding-condition wrapper steps of `in_macro_scope?` are implemented but not
exercised by any fixture.
