def foo
  # rubocop:disable Department/Cop
  return if condition
  # rubocop:enable Department/Cop

  bar
end
