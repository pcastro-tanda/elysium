# Style/IfUnlessModifier

Favor modifier if/unless usage when you have a single-line body.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for `if` and `unless` statements that would fit on one line if
written as modifier `if`/`unless`. The cop also checks for modifier
`if`/`unless` lines that exceed the maximum line length.

The maximum line length is configured in the `Layout/LineLength` cop.

One-line pattern matching is always allowed, since the match variable would
become undefined if the code were changed to the modifier form:

```ruby
if [42] in [x]
  x # `x` is undefined when using modifier form.
end
```

To respect the user's intention to use an endless method definition in the
`if` body, the following code is allowed:

```ruby
if condition
  def method_name = body
end
```

It is also allowed when a `defined?` argument has an undefined value, because
using the modifier form would change its result:

```ruby
unless defined?(undefined_foo)
  undefined_foo = 'default_value'
end
undefined_foo # => 'default_value'

undefined_bar = 'default_value' unless defined?(undefined_bar)
undefined_bar # => nil
```

```ruby
# bad
if condition
  do_stuff(bar)
end

unless qux.empty?
  Foo.do_something
end

do_something_with_a_long_name(arg) if long_condition_that_prevents_code_fit_on_single_line

# good
do_stuff(bar) if condition
Foo.do_something unless qux.empty?

if long_condition_that_prevents_code_fit_on_single_line
  do_something_with_a_long_name(arg)
end

if short_condition # a long comment that makes it too long if it were just a single line
  do_something
end
```

## Options

This rule has no options.

## Blind spots

Reads `Layout/LineLength`'s `Max`/`Enabled`/`AllowURI`/`AllowCopDirectives`/
`URISchemes`/`AllowedPatterns`/`IgnoredPatterns`, and
`Layout/IndentationStyle`'s `IndentationWidth`/`Layout/IndentationWidth`'s
`Width` (for weighing a line's leading tabs), all as peer options.

`Node#left_siblings` (used for the `defined?` guard and
`another_statement_on_same_line?`) and `Node#chained?`/`parenthesize?` (used
for the `chained?` guard and fix parenthesization) are reconstructed from a
`StatementsNode`'s direct body list and a few known wrapping constructs
(assignment to a local/instance/class/global/constant variable, `&&`/`||`,
array elements, hash values, call receiver/arguments) rather than true
parent pointers; an `if`/`unless` that is not a direct child of one of those
(e.g. inside a multiple assignment, an index write, or a `+=`/`||=`-style
operator assignment) is treated as having no left siblings and as never
needing parentheses or being chained, which only risks false negatives.

`if_body_source`'s omitted-hash-value reconstruction only special-cases a
call whose last argument is a hash/keyword-hash with a value-omitted last
pair (`obj.foo bar:`); other RuboCop-recognized shapes for that rewrite fall
back to the body's raw source, which is usually byte-identical anyway.

The heredoc-aware block-form correction (and `XStringNode`/
`InterpolatedXStringNode` heredocs) assumes a single-line heredoc body,
matching RuboCop's own `to_normal_form_with_heredoc`, which does not
re-indent a multi-line heredoc body per line either; only plain string and
interpolated-string heredocs are recognized as a call's last argument.

`AllowURI`'s URI matching is a simplified `scheme://\S+` regex rather than
`URI::DEFAULT_PARSER.make_regexp` plus RuboCop's YARD-link end-position
extension; it accepts the same URIs in the common case (a bare URI running
to the end of the line) but does not replicate the `{<uri> <title>}` or
trailing-word extensions.
