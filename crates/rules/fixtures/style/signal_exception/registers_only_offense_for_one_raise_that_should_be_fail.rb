map do
  raise 'I'
  ^^^^^ Use `fail` instead of `raise` to signal exceptions.
end.flatten.compact
