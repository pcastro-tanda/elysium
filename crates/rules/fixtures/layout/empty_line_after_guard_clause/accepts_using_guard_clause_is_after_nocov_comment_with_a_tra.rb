def foo
  # :nocov: legacy adapter
  return if condition
  # :nocov: legacy adapter

  bar
end
