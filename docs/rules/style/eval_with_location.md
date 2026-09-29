# Style/EvalWithLocation

Pass `__FILE__` and `__LINE__` to `eval` method, as they are used by backtraces.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Ensures that eval methods (`eval`, `instance_eval`, `class_eval` and
`module_eval`) are given filename and line number values (`__FILE__` and
`__LINE__`). This data is used to ensure that any errors raised within the
evaluated code will be given the correct identification in a backtrace.

The cop also checks that the line number given relative to `__LINE__` is
correct.

This cop will autocorrect incorrect or missing filename and line number
values. However, if `eval` is called without a binding argument, the cop
will not attempt to automatically add a binding, or add filename and line
values.

NOTE: This cop works only when a string literal is given as a code string.
No offense is reported if a string variable is given as below:

```ruby
code = <<-RUBY
  def do_something
  end
RUBY
eval code # not checked.
```

```ruby
# bad
eval <<-RUBY
  def do_something
  end
RUBY

# bad
C.class_eval <<-RUBY
  def do_something
  end
RUBY

# good
eval <<-RUBY, binding, __FILE__, __LINE__ + 1
  def do_something
  end
RUBY

# good
C.class_eval <<-RUBY, __FILE__, __LINE__ + 1
  def do_something
  end
RUBY
```

## Options

This rule has no options.

## Blind spots

Upstream's `check_line` only ever validates a line argument shaped as a bare
`__LINE__` or a `__LINE__ + N` call; any other call shape (notably
`__LINE__ - N`) is accepted unconditionally, correct or not. This port
preserves that literally rather than also validating minus forms.
