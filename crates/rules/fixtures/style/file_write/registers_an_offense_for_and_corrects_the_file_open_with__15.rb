File.open(filename, 'w+t') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  f.write(<<~EOS)
    content
  EOS
end
