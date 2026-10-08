# Sorbet/RedundantTLetForLiteral

Checks for redundant `T.let` declarations and trailing RBS annotations on constants whose literal values have types Sorbet can infer automatically.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for redundant `T.let` declarations and trailing RBS annotations where the assigned value is a literal whose type Sorbet can infer automatically.

Simple literals (strings, symbols, integers, floats, regexps) infer as their own class. Regexp literals are the only simple literals whose inference survives a `.freeze` call (Sorbet 0.6.13304+), so `T.let(/foo/.freeze, Regexp)` is also redundant; other frozen simple literals (e.g. `"hello".freeze`) are not inferred and still need `T.let`.

Array literals of simple literals are also inferred:

* A frozen array (`[...].freeze`) infers as a fixed-size tuple, which is a subtype of the annotated `T::Array`, so the annotation is redundant.
* An unfrozen array infers as `T::Array[<element type>]`. It is only flagged when that inferred type matches the annotation exactly, to avoid silently widening (e.g. `["a", nil]` infers a nilable element).

Hashes are excluded: Sorbet infers hash literals as `T.untyped`, so the annotation is required.

```ruby
# bad
MAX_RETRIES = T.let(3, Integer)
GREETING = T.let("hello", String)
SHELLS = T.let([:bash, :zsh].freeze, T::Array[Symbol])
RBS_GREETING = "hello" #: String

# good
MAX_RETRIES = 3
GREETING = "hello"
SHELLS = [:bash, :zsh].freeze
RBS_GREETING = "hello"
```

## Options

This rule has no options.

## Blind spots

None recorded.
