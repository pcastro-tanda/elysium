[1, 2, 3, 4]
  .map { |e| [e, e] }
   ^^^^^^^^^^^^^^^^^^ Use `flat_map` instead of `map...flatten`.
  .flatten(1)
  .size
