b = 1 + preceding_line.reduce(0) do |a, e|
  a + e.length + newline_length
end + 1
