array&.group_by(&:itself)&.transform_values(&:count)
       ^^^^^^^^ Use `tally` instead of `group_by` and `transform_values`.
