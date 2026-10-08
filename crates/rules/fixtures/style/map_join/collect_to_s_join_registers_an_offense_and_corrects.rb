array.collect(&:to_s).join(', ')
      ^^^^^^^ Remove redundant `collect(&:to_s)` before `join`.
