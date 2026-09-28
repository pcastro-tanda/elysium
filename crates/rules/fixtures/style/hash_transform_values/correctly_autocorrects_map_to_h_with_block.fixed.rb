{a: 1, b: 2}.transform_values {|v| foo(v)}.to_h {|k, v| [v, k]}
