Foo.left_outer_joins(:foo).where(foos: { id: nil }).where(bar: "bar")
    ^^^^^^^^^^^^^^^^^^^^^^ Use `where.missing(:foo)` instead of `left_outer_joins(:foo).where(foos: { id: nil })`.
