def foo
  return another_object if something_different?

  begin
    bar
  rescue SomeException
    baz
  end
end
