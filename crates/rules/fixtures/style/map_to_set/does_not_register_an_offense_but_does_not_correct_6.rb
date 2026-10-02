foo.collect { |x| x * 2 }.to_set { |x| [x.to_s, x] }
