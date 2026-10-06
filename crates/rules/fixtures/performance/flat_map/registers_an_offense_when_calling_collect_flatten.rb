[1, 2, 3, 4].collect { |e| [e, e] }.flatten
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `flat_map` instead of `collect...flatten`. Beware, `flat_map` only flattens 1 level and `flatten` can be used to flatten multiple levels.
