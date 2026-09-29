def foo
  @_my_var ||= :foo
  ^^^^^^^^ Memoized variable `@_my_var` does not match method name `foo`. Use `@_foo` instead.
end
