x = a.present? ? a.foo : nil
    ^^^^^^^^^^^^^^^^^^^^^^^^ Use `a.presence&.foo` instead of `a.present? ? a.foo : nil`.
