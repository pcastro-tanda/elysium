def foo
  return @_my_var if defined?(@_my_var)
                              ^^^^^^^^ Memoized variable `@_my_var` does not match method name `foo`. Use `@_foo` instead.
         ^^^^^^^^ Memoized variable `@_my_var` does not match method name `foo`. Use `@_foo` instead.
  @_my_var = false
  ^^^^^^^^ Memoized variable `@_my_var` does not match method name `foo`. Use `@_foo` instead.
end
