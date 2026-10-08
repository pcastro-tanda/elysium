File.open(filename, 'w+') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  f.write(<<~EOS)
    content
  EOS
end
