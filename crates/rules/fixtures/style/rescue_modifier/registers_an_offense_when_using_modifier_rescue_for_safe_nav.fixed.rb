begin
  obj&.method(<<~EOS)
  str
EOS
rescue
  handle
end
