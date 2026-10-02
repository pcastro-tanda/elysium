File.open(filename, 'w') { |f| f.write(<<~EOS) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  content
EOS
