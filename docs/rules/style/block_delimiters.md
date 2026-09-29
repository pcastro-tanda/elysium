# Style/BlockDelimiters

Avoid using {...} for multi-line blocks (multiline chaining is always ugly). Prefer {...} over do...end for single-line blocks.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of braces or do/end around single line or multi-line blocks.

Methods that can be either procedural or functional and cannot be
categorised from their usage alone is ignored. `lambda`, `proc`, and `it`
are their defaults. Additional methods can be added to `AllowedMethods`.

With `EnforcedStyle: line_count_based` (default), braces are preferred for
single-line blocks and `do...end` for multi-line ones:

```ruby
# bad - single line block
items.each do |item| item / 5 end

# good - single line block
items.each { |item| item / 5 }

# bad - multi-line block
things.map { |thing|
  something = thing.some_method
  process(something)
}

# good - multi-line block
things.map do |thing|
  something = thing.some_method
  process(something)
end
```

With `EnforcedStyle: semantic`, `do...end` is preferred for procedural
blocks (return value discarded) and `{...}` for functional ones (return
value used).

With `EnforcedStyle: braces_for_chaining`, braces are required around a
multi-line block whose return value is chained with another method call.

With `EnforcedStyle: always_braces`, braces are always required.

`BracesRequiredMethods` overrides every other configuration except
`AllowedMethods` and forces `{...}` for the listed method names.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `line_count_based` | `line_count_based`, `semantic`, `braces_for_chaining`, `always_braces` | The style of block delimiter to enforce. |
| ProceduralMethods | `benchmark`, `bm`, `bmbm`, `create`, `each_with_object`, `measure`, `new`, `realtime`, `tap`, `with_object` |  | Methods that are known to be procedural in nature but look functional from their usage, only used by the `semantic` style. |
| FunctionalMethods | `let`, `let!`, `subject`, `watch` |  | Methods that are known to be functional in nature but look procedural from their usage, only used by the `semantic` style. |
| AllowedMethods | `lambda`, `proc`, `it` |  | Methods that can be either procedural or functional and cannot be categorised from their usage alone. |
| AllowedPatterns | `[]` |  | Method name regex patterns always allowed to take either delimiter. |
| AllowBracesOnProceduralOneLiners | false |  | Whether a single-line procedural block may use braces, only used by the `semantic` style. |
| BracesRequiredMethods | `[]` |  | Method names that always require brace delimiters, overriding every other configuration except `AllowedMethods`. |

## Blind spots

`modifier_rescue?`/`require_do_end?` cannot distinguish a true `expr rescue expr2` modifier from
a keyword `rescue`/`end` clause with no exception class, no reference variable, a single resbody
and no `else` -- both shapes are treated as modifier-like, exactly mirroring the upstream
(possibly imprecise) heuristic rather than fixing it.
