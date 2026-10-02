File.open(filename, 'r+b') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.binread`.
  f.read
end
