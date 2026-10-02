File.open(filename, 'w+b') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.binwrite`.
  f.write(<<~EOS.gsub(/^/, ''))
    content
  EOS
end
