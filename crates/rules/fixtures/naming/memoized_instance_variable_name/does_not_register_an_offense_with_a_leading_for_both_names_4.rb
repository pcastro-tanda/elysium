def _foo
  return @_foo if defined?(@_foo)
  @_foo = false
end
