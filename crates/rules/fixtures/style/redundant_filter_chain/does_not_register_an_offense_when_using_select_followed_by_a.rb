arr.select(&:odd?).any?(Integer)
arr.select(&:odd?).any? { |x| x > 10 }
