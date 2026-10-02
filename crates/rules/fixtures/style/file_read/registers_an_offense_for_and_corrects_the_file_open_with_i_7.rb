File.open(filename, 'r+b') { |f| f.read }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.binread`.
