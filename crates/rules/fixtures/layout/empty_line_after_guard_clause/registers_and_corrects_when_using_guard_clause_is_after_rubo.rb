def foo
  return if condition
  ^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  # rubocop:disable Department/Cop
  bar
  # rubocop:enable Department/Cop
end
