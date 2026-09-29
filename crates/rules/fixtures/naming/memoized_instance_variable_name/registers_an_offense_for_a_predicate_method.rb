def foo?
  helper_variable = something_we_need_to_calculate_foo
  @bar ||= calculate_expensive_thing(helper_variable)
  ^^^^ Memoized variable `@bar` does not match method name `foo?`. Use `@foo` instead.
end
