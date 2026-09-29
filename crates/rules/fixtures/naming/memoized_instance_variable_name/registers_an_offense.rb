def x
  @my_var ||= :foo
  ^^^^^^^ Memoized variable `@my_var` does not match method name `x`. Use `@x` instead.
end
