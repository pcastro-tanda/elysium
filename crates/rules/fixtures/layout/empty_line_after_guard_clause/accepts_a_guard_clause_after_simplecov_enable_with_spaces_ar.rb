def foo
  # simplecov : disable
  return if condition
  # simplecov : enable

  bar
end
