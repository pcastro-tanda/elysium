Rails.env != :production
^^^^^^^^^^^^^^^^^^^^^^^^ Do not compare `Rails.env` with a symbol, it will always evaluate to `false`.
