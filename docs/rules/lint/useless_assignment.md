# Lint/UselessAssignment

Checks for useless assignment to a local variable.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for every useless assignment to local variable in every scope.
The basic idea for this cop was from the warning of `ruby -cw`:

```console
assigned but unused variable - foo
```

Currently this cop has advanced logic that detects unreferenced
reassignments and properly handles varied cases such as branch, loop,
rescue, ensure, etc.

This cop's autocorrection avoids cases like `a ||= 1` because removing
assignment from operator assignment can cause `NameError` if this
assignment has been used to declare a local variable.

NOTE: Given the assignment `foo = 1, bar = 2`, removing unused variables
can lead to a syntax error, so this case is not autocorrected.

```ruby
# bad
def some_method
  some_var = 1
  do_something
end

# good
def some_method
  some_var = 1
  do_something(some_var)
end
```

## Options

This rule has no options.

## Blind spots

The `Did you mean` suffix reimplements Ruby's `DidYouMean::SpellChecker`
(Jaro-Winkler plus Levenshtein); `sort_by` ties inside the spell checker are
broken by insertion order rather than by Ruby's unstable sort, so a
dictionary holding two equally similar names may suggest the other one.

RuboCop reuses one cop object across autocorrection passes, so the byte
ranges `IgnoredNode` collects in one pass keep suppressing offenses in the
next; this port rebuilds rule state per pass, so a chained assignment such
as `foo = bar = do_something` keeps being simplified until nothing is left
to remove instead of stopping after one round.
