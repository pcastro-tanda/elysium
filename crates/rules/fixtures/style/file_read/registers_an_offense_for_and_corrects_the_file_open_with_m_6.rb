File.open(filename, 'r+t') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.read`.
  f.read
end
