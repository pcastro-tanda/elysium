foo.map { |item| item if item.bar? }.compact
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Replace `map { ... }.compact` with `select`.
