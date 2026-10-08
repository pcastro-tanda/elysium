File.open(filename, 'w') do |f|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `File.write`.
  f.write(<<~HEAD + <<~TAIL)
    head
  HEAD
    tail
  TAIL
end
