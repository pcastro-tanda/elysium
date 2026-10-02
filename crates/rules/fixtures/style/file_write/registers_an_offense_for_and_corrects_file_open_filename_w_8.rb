::File.open(filename, 'w+b').write(content)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.binwrite`.
