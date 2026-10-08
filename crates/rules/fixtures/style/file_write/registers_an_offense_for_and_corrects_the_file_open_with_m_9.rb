File.open(filename, 'wb') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.binwrite`.
  f.write(<<~EOS.gsub(/^/, ''))
    content
  EOS
end
