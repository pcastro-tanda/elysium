foo.map { |x| x * 2 }.to_h { |x| [x.to_s, x] }
