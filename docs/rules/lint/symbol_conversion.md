# Lint/SymbolConversion

Checks for unnecessary symbol conversions.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for uses of literal strings converted to a symbol where a literal symbol could be used instead.

There are two possible styles for this cop. `strict` (default) will register an offense for any incorrect usage. `consistent` additionally requires hashes to use the same style for every symbol key (ie. if any symbol key needs to be quoted it requires all keys to be quoted).

```ruby
# bad
'string'.to_sym
:symbol.to_sym
'underscored_string'.to_sym
:'underscored_symbol'
'hyphenated-string'.to_sym
"string_#{interpolation}".to_sym

# good
:string
:symbol
:underscored_string
:underscored_symbol
:'hyphenated-string'
:"string_#{interpolation}"
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `strict` | `strict`, `consistent` | The preferred style when quoting symbol hash keys. |

## Blind spots

None recorded.
