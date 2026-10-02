Hash.new.filter { |x| x.match? /regexp/ }
Hash.new(:default).filter { |x| x.match? /regexp/ }
Hash.new { |hash, key| :default }.filter { |x| x.match? /regexp/ }
