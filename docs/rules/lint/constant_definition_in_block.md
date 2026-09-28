# Lint/ConstantDefinitionInBlock

Checks for the definition of constants within a block.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Do not define constants within a block, since the block's scope does not
isolate or namespace the constant in any way.

If you are trying to define that constant once, define it outside of the
block instead, or use a variable or method if defining the constant in the
outer scope would be problematic.

For meta-programming, use `const_set`.

```ruby
# bad
task :lint do
  FILES_TO_LINT = Dir['lib/*.rb']
end

# bad
describe 'making a request' do
  class TestRequest; end
end

# bad
module M
  extend ActiveSupport::Concern
  included do
    LIST = []
  end
end

# good
task :lint do
  files_to_lint = Dir['lib/*.rb']
end

# good
describe 'making a request' do
  let(:test_request) { Class.new }
  # see also `stub_const` for RSpec
end

# good
module M
  extend ActiveSupport::Concern
  included do
    const_set(:LIST, [])
  end
end
```

With `AllowedMethods: ['enums']` (the default):

```ruby
# good

# `enums` for Typed Enums via `T::Enum` in Sorbet.
# https://sorbet.org/docs/tenum
class TestEnum < T::Enum
  enums do
    Foo = new("foo")
  end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `enums` |  | Method names owning a block whose constants/classes/modules are never flagged. |

## Blind spots

Upstream's ancestor pattern does not recurse through intervening `if`/
`unless`/`kwbegin` between a constant/class/module and its enclosing block,
so `describe do; if x; FOO = 1; end; end` is never flagged on either side of
this port. `AllowedMethods` is matched only against the exact enclosing
block's owning method name (no `IgnoredMethods`/`ExcludedMethods` deprecated
aliases, which upstream also merges in only via the shared, but here unused,
`AllowedMethods` mixin default).
