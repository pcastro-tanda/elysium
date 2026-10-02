File.open(filename, 'wt') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  f.write(content)
end
