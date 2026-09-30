def foo
  begin
    return another_object if something_different?
  ensure
    bar
  end
end
