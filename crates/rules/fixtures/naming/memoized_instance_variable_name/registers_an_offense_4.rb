def x
  @y ||= begin
  ^^ Memoized variable `@y` does not match method name `x`. Use `@x` instead.
    :foo
  end
end
