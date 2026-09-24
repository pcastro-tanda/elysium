# Style/Documentation

Checks for missing top-level documentation of classes and modules.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | nursery |

Classes with no body are exempt from the check and so are namespace
modules - modules that have nothing in their bodies except classes, other
modules, constant definitions or constant visibility declarations.

The documentation requirement is annulled if the class or module has a
`#:nodoc:` comment next to it. Likewise, `#:nodoc: all` does the same for
all its children.

```ruby
# bad
class Person
  # ...
end

module Math
end

# good
# Description/Explanation of Person class
class Person
  # ...
end

# allowed
# Class without body
class Person
end

# Namespace - A namespace can be a class or a module
# Containing a class
module Namespace
  # Description/Explanation of Person class
  class Person
    # ...
  end
end

# Containing constant visibility declaration
module Namespace
  class Private
  end

  private_constant :Private
end

# Containing constant definition
module Namespace
  Public = Class.new
end

# Macro calls
module Namespace
  extend Foo
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedConstants | `[]` |  | Constant names, without namespace, exempted from the documentation check. |

## Blind spots

- Comment association is approximated as the contiguous run of full-line
  `#...` comments directly above the node's declaration line, instead of
  porting RuboCop's `Parser::Source::Comment::Associator`. This matches
  every ordinary leading-comment-block case (including a blank line or a
  trailing same-line comment on the previous statement breaking the run)
  but can differ for exotic comment placements the real associator would
  steal from an earlier sibling node.
- The `compact_namespace?` / `outer_module` special case (an RDoc-style
  `#:nodoc:` attached only to the outer segment of a compact `A::B::Test`
  path, found via a `(const (const nil? _) _)` node search) is not ported.
  Only the node's own declaration line and its actual `class`/`module`
  ancestors' declaration lines are checked for `:nodoc:`/`:nodoc: all`.
- `# rubocop:push`/`# rubocop:pop` directive comments are not recognized as
  directive comments (a `ruby_directives` limitation), so a preceding
  comment using them could be wrongly treated as documentation.
- The annotation-comment matcher approximates Ruby's
  `/^(# ?)(\b#{keywords}\b)(\s*:)?(\s+)?(\S+)?/i`: the margin allows at
  most one space between `#` and the keyword, matching the regex's `# ?`,
  but the exact backtracking RuboCop's regex engine performs when several
  keywords overlap as substrings of each other is only approximated by
  trying keywords longest-first.
