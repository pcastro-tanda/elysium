foo(<<~MSG.dup)
           ^^^ Don't unfreeze interpolated strings as they are already unfrozen.
  foo #{bar}
  baz
MSG
