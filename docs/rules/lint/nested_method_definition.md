# Lint/NestedMethodDefinition

Checks for nested method definitions.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

`bar` definition actually produces methods in the same scope as the outer
`foo` method. Furthermore, the `bar` method will be redefined every time
`foo` is invoked.

```ruby
# bad
def foo
  def bar
  end
end

# good
def foo
  bar = -> { puts 'hello' }
  bar.call
end

# good

# `class_eval`, `instance_eval`, `module_eval`, `class_exec`, `instance_exec`, and
# `module_exec` blocks are allowed by default.

def foo
  self.class.class_eval do
    def bar
    end
  end
end

def foo
  self.class.module_exec do
    def bar
    end
  end
end

# good

def foo
  class << self
    def bar
    end
  end
end
```

With `AllowedMethods: []` (the default):

```ruby
# bad
def do_something
  has_many :articles do
    def find_or_create_by_name(name)
    end
  end
end
```

With `AllowedMethods: ['has_many']`:

```ruby
# bad
def do_something
  has_many :articles do
    def find_or_create_by_name(name)
    end
  end
end
```

With `AllowedPatterns: []` (the default):

```ruby
# bad
def foo(obj)
  obj.do_baz do
    def bar
    end
  end
end
```

With `AllowedPatterns: ['baz']`:

```ruby
# good
def foo(obj)
  obj.do_baz do
    def bar
    end
  end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `[]` |  | Method names owning a block whose nested method definitions are never flagged. |
| AllowedPatterns | `[]` |  | Method name regex patterns owning a block whose nested method definitions are never flagged. |

## Blind spots

Upstream's `each_ancestor(:any_block, :sclass).any?` scan considers every enclosing block/sclass
ancestor anywhere up the tree, not merely those between the flagged def and its nearest enclosing
def; this port reproduces that exactly, so a scoping block *outside* (above) the nearest enclosing
def also suppresses the offense, matching upstream. A `def` written as an argument expression
(`foo(def bar; end)`) is not specially handled, matching upstream's own lack of a special case.
