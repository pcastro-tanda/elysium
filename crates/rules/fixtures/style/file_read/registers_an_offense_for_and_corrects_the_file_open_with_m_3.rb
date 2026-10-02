File.open(filename, 'rt') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.read`.
  f.read
end
