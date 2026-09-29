define_method(:values) do
  @foo ||= do_something
  ^^^^ Memoized variable `@foo` does not match method name `values`. Use `@values` instead.
end
