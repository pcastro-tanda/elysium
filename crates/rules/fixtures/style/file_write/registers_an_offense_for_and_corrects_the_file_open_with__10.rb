File.open(filename, 'w+') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  f.write(content)
end
