Foo.left_joins(:foo).where(foo: { id: nil }).where(bar: "bar")
    ^^^^^^^^^^^^^^^^ Use `where.missing(:foo)` instead of `left_joins(:foo).where(foo: { id: nil })`.
