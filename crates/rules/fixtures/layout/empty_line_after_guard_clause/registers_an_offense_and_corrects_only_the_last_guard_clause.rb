def foo
  return if foo?
  return if bar?
  ^^^^^^^^^^^^^^ Add empty line after guard clause.
  foobar
end
