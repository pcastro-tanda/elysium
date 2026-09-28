{a: 1, b: 2}.to_h { |k, v| [k.to_sym, foo(v)] }
