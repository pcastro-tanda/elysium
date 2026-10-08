Dir.each_child('path/to/dir').none? { |f| f.start_with?('.') }
