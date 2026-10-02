array.group_by(&:itself).transform_values(&:size)
      ^^^^^^^^ Use `tally` instead of `group_by` and `transform_values`.
