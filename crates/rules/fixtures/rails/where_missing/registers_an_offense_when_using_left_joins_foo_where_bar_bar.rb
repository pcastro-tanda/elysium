Foo.left_joins(:foo).where(bar: "bar", foos: { id: nil })
    ^^^^^^^^^^^^^^^^ Use `where.missing(:foo)` instead of `left_joins(:foo).where(foos: { id: nil })`.
