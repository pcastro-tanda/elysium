[1, 2, 3, 4]
  .collect(&:foo)
   ^^^^^^^^^^^^^^ Use `flat_map` instead of `collect...flatten`.
  .flatten(1)
  .size
