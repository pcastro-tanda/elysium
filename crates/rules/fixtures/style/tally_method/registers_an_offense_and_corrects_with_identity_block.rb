array.group_by { |x| x }.transform_values(&:count)
      ^^^^^^^^ Use `tally` instead of `group_by` and `transform_values`.
