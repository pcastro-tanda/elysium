File.open(filename, 'w+b') { |f| f.write(content) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.binwrite`.
