foo = def x
  @y ||= :foo
  ^^ Memoized variable `@y` does not match method name `x`. Use `@x` instead.
end
