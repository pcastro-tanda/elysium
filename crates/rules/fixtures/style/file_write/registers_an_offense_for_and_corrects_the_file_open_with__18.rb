File.open(filename, 'w+b') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.binwrite`.
  f.write(content)
end
