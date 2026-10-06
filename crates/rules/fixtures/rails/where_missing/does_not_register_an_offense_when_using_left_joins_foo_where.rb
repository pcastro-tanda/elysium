Foo.left_joins(:foo).where(foos: { id: nil }).where(bar: "bar")
