x.starts_with?(a, b) || x.starts_with?("c", D)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `x.starts_with?(a, b, "c", D)` instead of `x.starts_with?(a, b) || x.starts_with?("c", D)`.
