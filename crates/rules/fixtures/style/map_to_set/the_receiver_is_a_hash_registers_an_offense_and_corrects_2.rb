{ foo: :bar }.collect { |x, y| [x.to_s, y.to_s] }.to_set
              ^^^^^^^ Pass a block to `to_set` instead of calling `collect.to_set`.
