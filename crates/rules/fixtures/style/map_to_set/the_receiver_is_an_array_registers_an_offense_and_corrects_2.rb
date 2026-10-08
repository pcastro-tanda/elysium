[1, 2, 3].collect { |x| [x, x * 2] }.to_set
          ^^^^^^^ Pass a block to `to_set` instead of calling `collect.to_set`.
