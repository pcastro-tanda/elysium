array.map { _1.to_s }.join(', ')
      ^^^ Remove redundant `map(&:to_s)` before `join`.
