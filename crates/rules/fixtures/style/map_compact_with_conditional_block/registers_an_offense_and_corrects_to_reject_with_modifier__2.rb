foo.map { |item| item unless item.bar? }.compact
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Replace `map { ... }.compact` with `reject`.
