x.group_by { |e| e.type }.transform_values {|v| foo(v)}
