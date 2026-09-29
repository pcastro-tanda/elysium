def foo
  return @foo if defined?(@foo)
                          ^^^^ Memoized variable `@foo` does not start with `_`. Use `@_foo` instead.
         ^^^^ Memoized variable `@foo` does not start with `_`. Use `@_foo` instead.
  @foo = false
  ^^^^ Memoized variable `@foo` does not start with `_`. Use `@_foo` instead.
end
