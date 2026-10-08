array.each_with_object(Hash.new(0)) { _2[_1.to_s] += 1 }
