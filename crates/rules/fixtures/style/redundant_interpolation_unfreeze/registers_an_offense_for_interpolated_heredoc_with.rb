foo(+<<~MSG)
    ^ Don't unfreeze interpolated strings as they are already unfrozen.
  foo #{bar}
  baz
MSG
