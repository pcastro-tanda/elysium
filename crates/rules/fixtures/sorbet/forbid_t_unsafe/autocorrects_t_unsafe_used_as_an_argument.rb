consume(T.unsafe(foo))
        ^^^^^^^^^^^^^ Do not use `T.unsafe`.
consume(first, T.unsafe(foo), last)
               ^^^^^^^^^^^^^ Do not use `T.unsafe`.
consume(value: T.unsafe(foo))
               ^^^^^^^^^^^^^ Do not use `T.unsafe`.
consume(first, T.unsafe("already #: marker"), last)
               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not use `T.unsafe`.
