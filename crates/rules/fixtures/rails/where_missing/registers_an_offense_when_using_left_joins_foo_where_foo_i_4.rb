Foo.left_joins(:foo).where(bar: "bar").and(Foo.where(foos: {id: nil}).left_joins(:foo))
                                                                      ^^^^^^^^^^^^^^^^ Use `where.missing(:foo)` instead of `left_joins(:foo).where(foos: { id: nil })`.
