def foo
  # rubocop:disable Department/Cop
  return if condition
  ^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  # rubocop:enable Department/Cop
  bar
end
