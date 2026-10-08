Hash.new.reject { |x| x.match? /regexp/ }
Hash.new(:default).reject { |x| x.match? /regexp/ }
Hash.new { |hash, key| :default }.reject { |x| x.match? /regexp/ }
