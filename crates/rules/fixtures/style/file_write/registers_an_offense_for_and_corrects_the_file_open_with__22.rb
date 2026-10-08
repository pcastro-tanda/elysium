File.open(filename, 'w') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  f.write('prefix' + <<~EOS)
    content
  EOS
end
