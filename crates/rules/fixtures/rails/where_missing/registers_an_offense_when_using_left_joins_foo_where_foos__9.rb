def test
  Foo.left_joins(:foo).any?

  do_something

  Foo.left_joins(:foo).where(foos: {id: nil})
      ^^^^^^^^^^^^^^^^ Use `where.missing(:foo)` instead of `left_joins(:foo).where(foos: { id: nil })`.
end
