def test
  Foo.left_joins(:foo).any?

  do_something

  Foo.where.missing(:foo)
end
