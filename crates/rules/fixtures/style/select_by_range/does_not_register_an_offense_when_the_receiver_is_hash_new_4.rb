Hash.new.reject { |x| x.between?(1, 10) }
Hash.new(:default).reject { |x| x.between?(1, 10) }
