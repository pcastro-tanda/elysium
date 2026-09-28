x.group_by { |e| e.type }.transform_keys {|k| foo(k)}
