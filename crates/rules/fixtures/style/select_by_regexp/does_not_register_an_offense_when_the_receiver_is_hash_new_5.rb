Hash.new.find_all { |x| x.match? /regexp/ }
Hash.new(:default).find_all { |x| x.match? /regexp/ }
Hash.new { |hash, key| :default }.find_all { |x| x.match? /regexp/ }
