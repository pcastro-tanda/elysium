begin
  something
  return
rescue EOFError, ArgumentError => e
  file.close
end
