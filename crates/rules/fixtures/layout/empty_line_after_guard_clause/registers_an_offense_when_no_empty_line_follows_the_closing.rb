def foo
  # simplecov:disable
  return if condition
  ^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  # simplecov:enable
  bar
end
