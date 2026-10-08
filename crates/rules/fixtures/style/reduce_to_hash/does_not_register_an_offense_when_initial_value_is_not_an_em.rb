array.each_with_object(Hash.new(0)) { |elem, hash| hash[elem] += 1 }
