File.open(filename, 'wb') { |f| f.write('hello') }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.binwrite`.
