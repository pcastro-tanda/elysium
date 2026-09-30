def foo
  begin
    bar
  rescue SomeException
    return another_object if something_different?
  else
    bar
  end
end
