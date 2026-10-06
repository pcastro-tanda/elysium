Foo.left_joins(:foo).where(foos: { id: nil }, bar: "bar")
    ^^^^^^^^^^^^^^^^ Use `where.missing(:foo)` instead of `left_joins(:foo).where(foos: { id: nil })`.
