Foo.left_joins(:foo).where(foos: {id: nil}).or(Foo.where(bar: "bar").left_joins(:foo))
    ^^^^^^^^^^^^^^^^ Use `where.missing(:foo)` instead of `left_joins(:foo).where(foos: { id: nil })`.
