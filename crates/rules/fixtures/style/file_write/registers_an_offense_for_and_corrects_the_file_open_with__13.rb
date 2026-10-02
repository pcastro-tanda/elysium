File.open(filename, 'w+t') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  f.write(content)
end
