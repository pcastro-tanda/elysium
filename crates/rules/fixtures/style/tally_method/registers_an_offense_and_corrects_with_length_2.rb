array.group_by { |x| x }.transform_values { |v| v.length }
      ^^^^^^^^ Use `tally` instead of `group_by` and `transform_values`.
