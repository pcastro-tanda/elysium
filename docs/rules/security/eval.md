# Security/Eval

Checks for the use of `Kernel#eval` and `Binding#eval`.

| | |
| --- | --- |
| Department | Security |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for the use of `Kernel#eval` and `Binding#eval`.

```ruby
# bad

eval(something)
binding.eval(something)
Kernel.eval(something)
```

## Options

This rule has no options.

## Blind spots

Upstream's `recursive_literal?` also treats a call to one of a fixed set of
comparison-ish methods (`==`, `!=`, `<`, `>`, `<=`, `>=`, `<=>`, `*`, `!`) as
literal when its receiver and arguments are themselves all literal (e.g. an
interpolation like `"#{1 == 1}"`). This port only walks composite literal
node kinds, `and`/`or`, and nested statement bodies, so that one shape is
treated as non-literal (flagged) where upstream would accept it. No fixture
in this corpus exercises it.
