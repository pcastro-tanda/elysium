Foo.left_joins(:foo).where(bazs: { id: nil })
Foo.left_joins(:foobar).where(foo: { id: nil })
Foo.left_joins(:foo).where(foobar: { id: nil })
