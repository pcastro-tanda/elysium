array.map(&:to_s).join
      ^^^ Remove redundant `map(&:to_s)` before `join`.
