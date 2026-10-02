array.group_by { _1 }.transform_values(&:count)
      ^^^^^^^^ Use `tally` instead of `group_by` and `transform_values`.
