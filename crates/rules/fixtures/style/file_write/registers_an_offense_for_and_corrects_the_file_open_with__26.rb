File.open(<<~PATH.strip, 'w') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  path/to/file
PATH
  f.write(content)
end
