Foo.left_joins(:foo).or(Foo.where(foos: {id: nil}))
