Foo.left_joins(foo: :bar).where(bars: { id: nil })
Foo.left_joins(bar: :foo).where(bars: { id: nil })
