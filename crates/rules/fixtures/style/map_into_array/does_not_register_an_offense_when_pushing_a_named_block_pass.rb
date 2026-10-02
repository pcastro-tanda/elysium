dest = []
src.each { |e| dest.push(&method(:foo)) }
