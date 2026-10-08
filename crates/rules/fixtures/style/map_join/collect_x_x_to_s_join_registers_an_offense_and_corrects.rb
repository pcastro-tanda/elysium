array.collect { |x| x.to_s }.join(', ')
      ^^^^^^^ Remove redundant `collect(&:to_s)` before `join`.
