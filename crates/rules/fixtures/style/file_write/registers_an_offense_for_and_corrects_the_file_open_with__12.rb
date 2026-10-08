File.open(filename, 'w+') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  f.write(<<~EOS.gsub(/^/, ''))
    content
  EOS
end
