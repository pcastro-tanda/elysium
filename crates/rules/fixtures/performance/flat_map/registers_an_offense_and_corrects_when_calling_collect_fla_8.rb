[1, 2, 3, 4]
  .collect { |e| [e, e] }
   ^^^^^^^^^^^^^^^^^^^^^^ Use `flat_map` instead of `collect...flatten!`.
  .flatten!(1)
  .size
