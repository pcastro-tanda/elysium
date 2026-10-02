File.open(filename, 'rb') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.binread`.
  f.read
end
