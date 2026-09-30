def foo
  object.tap { |obj| return another_object if something? }
  foobar
end
