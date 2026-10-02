arr.filter(&:odd?).any?(Integer)
arr.filter(&:odd?).any? { |x| x > 10 }
