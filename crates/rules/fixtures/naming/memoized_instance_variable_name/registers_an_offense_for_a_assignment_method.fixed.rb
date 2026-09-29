def foo=(bar)
  helper_variable = something_we_need_to_calculate(foo)
  @foo ||= calculate_expensive_thing(helper_variable)
end
