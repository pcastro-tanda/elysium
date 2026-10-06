def test
  Foo.where.missing(:foo)

  do_something

  Foo.where.missing(:foo)
end
