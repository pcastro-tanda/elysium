{a: 1, b: 2}.transform_keys {|k| k.to_s}.to_h {|k, v| [v, k]}
