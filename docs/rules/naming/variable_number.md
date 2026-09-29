# Naming/VariableNumber

Use the configured style when numbering symbols, methods and variables.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

```ruby
# EnforcedStyle: normalcase (default)

# bad
:some_sym_1
variable_1 = 1

def some_method_1; end

def some_method1(arg_1); end

# good
:some_sym1
variable1 = 1

def some_method1; end

def some_method1(arg1); end
```

```ruby
# EnforcedStyle: snake_case

# bad
:some_sym1
variable1 = 1

def some_method1; end

def some_method_1(arg1); end

# good
:some_sym_1
variable_1 = 1

def some_method_1; end

def some_method_1(arg_1); end
```

```ruby
# EnforcedStyle: non_integer

# bad
:some_sym1
:some_sym_1

variable1 = 1
variable_1 = 1

def some_method1; end

def some_method_1; end

def some_methodone(arg1); end
def some_methodone(arg_1); end

# good
:some_symone
:some_sym_one

variableone = 1
variable_one = 1

def some_methodone; end

def some_method_one; end

def some_methodone(argone); end
def some_methodone(arg_one); end
```

```ruby
# AllowedIdentifiers: [capture3]

# good
expect(Open3).to receive(:capture3)
```

```ruby
# AllowedPatterns: ['_v\d+\z']

# good
:some_sym_v1
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `normalcase` | `snake_case`, `normalcase`, `non_integer` | Naming style numbered symbols, methods and variables must follow. |
| CheckMethodNames | true |  | Whether method names are checked. |
| CheckSymbols | true |  | Whether symbols are checked. |
| AllowedIdentifiers | `TLS1_1`, `TLS1_2`, `capture3`, `iso8601`, `rfc1123_date`, `rfc822`, `rfc2822`, `rfc3339`, `x86_64` |  | Identifiers (sigils stripped) that are never checked. |
| AllowedPatterns | `[]` |  | Regexps; an identifier matching one is accepted regardless of style. |

## Blind spots

The `class_emitter_method?` escape hatch (a singleton method named after a sibling class, e.g. `def self.Foo1; end` next to `class Foo1; end`) is not implemented; such a method is flagged even though upstream would accept it.
