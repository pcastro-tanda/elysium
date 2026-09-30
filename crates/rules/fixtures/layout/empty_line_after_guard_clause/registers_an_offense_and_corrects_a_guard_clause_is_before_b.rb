def foo
  return another_object if something_different?
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  begin
    bar
  rescue SomeException
    baz
  end
end
