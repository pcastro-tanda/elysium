foo.map { |item| item.bar? ? next : item }.compact
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Replace `map { ... }.compact` with `reject`.
