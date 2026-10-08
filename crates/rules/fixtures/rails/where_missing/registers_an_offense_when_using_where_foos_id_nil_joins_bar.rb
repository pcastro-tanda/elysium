Foo.left_joins(:foo).joins(:bar).where(foos: { id: nil })
    ^^^^^^^^^^^^^^^^ Use `where.missing(:foo)` instead of `left_joins(:foo).where(foos: { id: nil })`.
