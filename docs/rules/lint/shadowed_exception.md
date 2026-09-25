# Lint/ShadowedException

Checks for a rescued exception that get shadowed by a less specific exception being rescued before a more specific exception is rescued.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

An exception is considered shadowed if it is rescued after its
ancestor is, or if it and its ancestor are both rescued in the
same `rescue` statement. In both cases, the more specific rescue is
unnecessary because it is covered by rescuing the less specific
exception. (ie. `rescue Exception, StandardError` has the same behavior
whether `StandardError` is included or not, because all `StandardError`s
are rescued by `rescue Exception`).

```ruby
# bad

begin
  something
rescue Exception
  handle_exception
rescue StandardError
  handle_standard_error
end

# bad
begin
  something
rescue Exception, StandardError
  handle_error
end

# good

begin
  something
rescue StandardError
  handle_standard_error
rescue Exception
  handle_exception
end

# good, however depending on runtime environment.
#
# This is a special case for system call errors.
# System dependent error code depends on runtime environment.
# For example, whether `Errno::EAGAIN` and `Errno::EWOULDBLOCK` are
# the same error code or different error code depends on environment.
# This good case is for `Errno::EAGAIN` and `Errno::EWOULDBLOCK` with
# the same error code.
begin
  something
rescue Errno::EAGAIN, Errno::EWOULDBLOCK
  handle_standard_error
end
```

## Options

This rule has no options.

## Blind spots

Exception resolution is a static lookup against a table of every built-in
`c < Exception` class generated on one real Ruby process (see
`EXCEPTION_HIERARCHY`'s doc comment), not an actual `Kernel.const_get`:
custom exception classes defined elsewhere in the same codebase (or
reopened core classes) are always unresolved, exactly as they would be in
a RuboCop run where that file was never `require`d -- this only differs
from a full-project RuboCop run when the linted file itself defines and
immediately rescues its own custom hierarchy in a way that would already
be loaded by the time the cop's process resolves constants, which
RuboCop's own architecture (one process, `require`s happen via the
target application's boot, not the linter) makes exceedingly rare.
`Errno::*` pairs are never flagged for containing multiple levels
regardless of real platform error-code aliasing, matching every reachable
case of upstream's own `system_call_err?` special-casing (see the module
doc comment for the proof).
