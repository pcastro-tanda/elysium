Foo.left_outer_joins(:foo).where(foos: { id: nil }).where(bar: "bar")
