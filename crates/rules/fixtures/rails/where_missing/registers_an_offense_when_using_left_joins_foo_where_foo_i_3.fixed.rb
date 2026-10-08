Foo.where.missing(:foo).or(Foo.where(bar: "bar").left_joins(:foo))
