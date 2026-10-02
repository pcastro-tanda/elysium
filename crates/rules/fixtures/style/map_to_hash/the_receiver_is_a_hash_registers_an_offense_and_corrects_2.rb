{ foo: :bar }.collect { |x, y| [x.to_s, y.to_s] }.to_h
              ^^^^^^^ Pass a block to `to_h` instead of calling `collect.to_h`.
