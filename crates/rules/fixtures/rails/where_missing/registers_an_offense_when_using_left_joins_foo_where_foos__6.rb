Foo.
  left_joins(:foo).
  ^^^^^^^^^^^^^^^^ Use `where.missing(:foo)` instead of `left_joins(:foo).where(foos: { id: nil })`.
  where(foos: { id: nil }).
  where(bar: "bar")
