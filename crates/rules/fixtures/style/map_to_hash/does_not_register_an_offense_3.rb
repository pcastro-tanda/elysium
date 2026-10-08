foo.map { |x| x * 2 }.to_h { [it.to_s, it] }
