array.each_with_object(Hash.new(0)) { |item, counts| counts[item] += 2 }
