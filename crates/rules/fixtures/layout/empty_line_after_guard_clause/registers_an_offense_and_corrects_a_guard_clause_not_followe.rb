def foo
  return if need_return?
  ^^^^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  foobar
end
