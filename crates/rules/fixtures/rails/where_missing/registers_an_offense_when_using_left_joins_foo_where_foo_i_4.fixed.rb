Foo.left_joins(:foo).where(bar: "bar").and(Foo.where.missing(:foo))
