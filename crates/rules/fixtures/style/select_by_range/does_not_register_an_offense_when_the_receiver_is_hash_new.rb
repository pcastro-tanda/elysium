Hash.new.select { |x| x.between?(1, 10) }
Hash.new(:default).select { |x| x.between?(1, 10) }
