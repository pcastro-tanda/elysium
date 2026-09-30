def foo
  # :nocov:
  return if condition
  # :nocov:

  bar
end
