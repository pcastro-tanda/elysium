content = 'hello'
File.open(filename, 'w') { |f| f.write(content) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
