def foo
  @foo ||= :foo
  ^^^^ Memoized variable `@foo` does not start with `_`. Use `@_foo` instead.
end
