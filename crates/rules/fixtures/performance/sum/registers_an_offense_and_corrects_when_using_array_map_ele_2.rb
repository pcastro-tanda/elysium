array&.map { |elem| elem ** 2 }&.sum
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `sum { ... }` instead of `map { ... }&.sum`.
