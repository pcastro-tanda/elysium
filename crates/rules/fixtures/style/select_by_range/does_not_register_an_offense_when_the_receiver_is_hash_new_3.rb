Hash.new.filter { |x| x.between?(1, 10) }
Hash.new(:default).filter { |x| x.between?(1, 10) }
