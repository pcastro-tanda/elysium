def _foo
  return @foo if defined?(@foo)
  @foo = false
end
