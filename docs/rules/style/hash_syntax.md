# Style/HashSyntax

Prefer Ruby 1.9 hash syntax `{ a: 1, b: 2 }` over 1.8 syntax `{ :a => 1, :b => 2 }`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

Checks hash literal syntax.

It can enforce either the use of the classic hash rocket syntax or the use
of the newer Ruby 1.9 syntax (when applicable).

A separate offense is registered for each problematic pair.

* `ruby19` (default) - forces use of the 1.9 syntax (e.g. `{a: 1}`) when
  hashes have all symbols for keys.
* `hash_rockets` - forces use of hash rockets for all hashes.
* `no_mixed_keys` - simply checks for hashes with mixed syntaxes.
* `ruby19_no_mixed_keys` - forces use of ruby 1.9 syntax and forbids mixed
  syntax hashes.

```ruby
# EnforcedStyle: ruby19 (default)
# bad
{:a => 2}
{b: 1, :c => 2}

# good
{a: 2, b: 1}
{:c => 2, 'd' => 2} # acceptable since 'd' isn't a symbol
{d: 1, 'e' => 2} # technically not forbidden
```

This cop also has an `EnforcedShorthandSyntax` option for Ruby 3.1's hash
value omission syntax (default `either`):

* `always` - forces use of the 3.1 syntax (e.g. `{foo:}`).
* `never` - forces use of explicit hash literal value.
* `either` - accepts both shorthand and explicit use of hash literal value.
* `consistent` - forces use of the 3.1 syntax only if all values can be
  omitted in the hash.
* `either_consistent` - accepts both shorthand and explicit use of hash
  literal value, but they must be consistent within a hash.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `ruby19` | `ruby19`, `hash_rockets`, `no_mixed_keys`, `ruby19_no_mixed_keys` | Which hash key syntax to enforce. |
| EnforcedShorthandSyntax | `either` | `always`, `never`, `either`, `consistent`, `either_consistent` | Whether to enforce Ruby 3.1's hash value shorthand (`{foo:}`). |
| UseHashRocketsWithSymbolValues | false |  | Force hashes that have a symbol value to use hash rockets. |
| PreferHashRocketsForNonAlnumEndingSymbols | false |  | Do not suggest `{ a: 1 }` over `{ :a? => 1 }` in ruby19 style. |

## Blind spots

`acceptable_19_syntax_symbol?`'s identifier check uses ASCII
`[A-Za-z_]\w*[?!]?`, not Ruby's Unicode-aware `\w`: a non-ASCII symbol name
is treated as unacceptable for ruby19 conversion (kept as a hash rocket),
which is the safe (false-negative) direction, not RuboCop's exact behavior.
Likewise `PreferHashRocketsForNonAlnumEndingSymbols`'s `\p{Alnum}` check
treats any non-ASCII byte as alnum rather than checking the actual Unicode
category.

The `EnforcedShorthandSyntax` mixin's parenthesization logic
(`def_node_that_require_parentheses`, `last_expression?`,
`method_dispatch_as_argument?`) is re-derived here from the ancestor chain
`Context::ancestors` gives for free at the hash's own visit, plus a small
per-file cache of facts about `Call`/`Super`/`Yield`/modifier-conditional/
parentheses-group/assignment-writer nodes (see the module docs), rather
than RuboCop-AST's live `node.parent`/`node.right_sibling`/`each_ancestor`;
it matches RuboCop's cop-spec-verified cases but has not been proven
against every possible nesting. In particular, `right_sibling?` is only
tracked for direct `StatementsNode` items (matching every real body:
method/block bodies, `if`/`unless`/`while`/`until`/`begin`/`rescue`
branches, since Prism always wraps them); an ancestor call or assignment
that instead sits as, e.g., an array element or a hash value is treated as
having no right sibling, which only risks the false-negative direction
(skipping a parenthesization that RuboCop would still consider safe to
skip in that position anyway, or emitting one RuboCop's mixin would already
render unambiguous).

`TargetRubyVersion` is read via `AllCops` peer options for the `<= 3.0`
(shorthand syntax unsupported) and `<= 2.1` (quoted-symbol ruby19 syntax
unsupported) gates; when absent, both default to a modern Ruby (shorthand
enabled, quoted symbols allowed), matching RuboCop's own
`DEFAULT_RUBY_VERSION`.

When a hash with multiple shorthand-omittable pairs needs its enclosing
bare call wrapped in parentheses, only the pair that is itself the hash's
last pair carries the paren-adding edits (RuboCop's rewriter would merge
identical edits from every offending pair's own correction block; this
engine's fixes are independent, so attaching them to every pair would make
every pair but one's fix collide and get dropped instead).
