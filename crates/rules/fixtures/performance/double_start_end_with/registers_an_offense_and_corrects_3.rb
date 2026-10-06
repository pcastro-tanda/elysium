!x.start_with?(a, b) && !x.start_with?("c", D)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `!x.start_with?(a, b, "c", D)` instead of `!x.start_with?(a, b) && !x.start_with?("c", D)`.
