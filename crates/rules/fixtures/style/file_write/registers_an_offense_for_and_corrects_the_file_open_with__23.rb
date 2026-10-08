File.open(filename, 'w') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  f.write(process(<<~EOS))
    content
  EOS
end
