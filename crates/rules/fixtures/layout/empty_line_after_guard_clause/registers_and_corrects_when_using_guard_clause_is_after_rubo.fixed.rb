def foo
  return if condition

  # rubocop:disable Department/Cop
  bar
  # rubocop:enable Department/Cop
end
