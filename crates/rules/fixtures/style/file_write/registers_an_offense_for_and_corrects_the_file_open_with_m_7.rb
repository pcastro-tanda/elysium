File.open(filename, 'wb') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.binwrite`.
  f.write(content)
end
