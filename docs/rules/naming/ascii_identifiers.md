# Naming/AsciiIdentifiers

Use only ascii symbols in identifiers and constants.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for non-ascii characters in identifier and constant names.
Identifiers are always checked and whether constants are checked
can be controlled using AsciiConstants config.

```ruby
# bad
def καλημερα # Greek alphabet (non-ascii)
end

# bad
def こんにちはと言う # Japanese character (non-ascii)
end

# bad
def hello_🍣 # Emoji (non-ascii)
end

# good
def say_hello
end

# bad
신장 = 10 # Hangul character (non-ascii)

# good
height = 10
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AsciiConstants | true |  | Whether to check constant names too. |

## Blind spots

Upstream also flags non-ascii characters in symbol literals (whose bare
content lexes as a `tIDENTIFIER`/`tCONSTANT` token) and treats bare `@ivar`/
`$gvar`/`@@cvar` names as never-checked (they are not `tIDENTIFIER`/
`tCONSTANT` tokens either). This port does not visit `SymbolNode`, matching
the never-checked instance/global/class-variable behavior but missing the
symbol-literal case.
