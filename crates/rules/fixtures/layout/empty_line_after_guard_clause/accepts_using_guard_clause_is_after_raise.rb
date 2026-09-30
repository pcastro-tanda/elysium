def foo
  raise ArgumentError, 'HTTP redirect too deep' if limit.zero?

  foobar
end
