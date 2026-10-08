array.group_by { |x| x.name }.transform_values(&:count)
