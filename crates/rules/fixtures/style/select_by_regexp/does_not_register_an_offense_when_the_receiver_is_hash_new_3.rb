Hash.new.select { |x| x.match? /regexp/ }
Hash.new(:default).select { |x| x.match? /regexp/ }
Hash.new { |hash, key| :default }.select { |x| x.match? /regexp/ }
