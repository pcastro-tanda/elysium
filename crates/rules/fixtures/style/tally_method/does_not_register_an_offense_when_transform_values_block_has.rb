array.group_by { |x| x }.transform_values { |v| v.count * 2 }
