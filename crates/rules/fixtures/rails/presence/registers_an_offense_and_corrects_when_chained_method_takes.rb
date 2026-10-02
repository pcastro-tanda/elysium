a.present? ? a.foo(42, key: :value) : nil
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `a.presence&.foo(42, key: :value)` instead of `a.present? ? a.foo(42, key: :value) : nil`.
