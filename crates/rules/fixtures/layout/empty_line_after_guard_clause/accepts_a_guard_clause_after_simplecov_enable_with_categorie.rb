def foo
  # simplecov:disable line, branch legacy adapter
  return if condition
  # simplecov:enable line, branch legacy adapter

  bar
end
