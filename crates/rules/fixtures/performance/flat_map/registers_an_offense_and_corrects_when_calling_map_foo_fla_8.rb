[1, 2, 3, 4]
  .map(&:foo)
   ^^^^^^^^^^ Use `flat_map` instead of `map...flatten`.
  .flatten(1)
  .size
