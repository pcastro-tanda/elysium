File.open(filename, 'r+') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.read`.
  f.read
end
