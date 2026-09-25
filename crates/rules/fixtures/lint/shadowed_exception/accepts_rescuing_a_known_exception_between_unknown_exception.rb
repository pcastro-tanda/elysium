begin
  a
rescue UnknownException
  b
rescue StandardError
  c
rescue AnotherUnknownException
  d
end
