Hash.new.find_all { |x| x.between?(1, 10) }
Hash.new(:default).find_all { |x| x.between?(1, 10) }
