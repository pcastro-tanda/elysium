x.ends_with?(a, b) || x.ends_with?("c", D)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `x.ends_with?(a, b, "c", D)` instead of `x.ends_with?(a, b) || x.ends_with?("c", D)`.
