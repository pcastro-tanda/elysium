def foo
  begin
    return another_object if something_different?
  rescue SomeException
    bar
  end
end
