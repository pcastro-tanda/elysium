arr.find_all(&:odd?).any?(Integer)
arr.find_all(&:odd?).any? { |x| x > 10 }
