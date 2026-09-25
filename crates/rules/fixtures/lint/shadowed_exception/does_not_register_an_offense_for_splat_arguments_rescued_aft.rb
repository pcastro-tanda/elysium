begin
  a
rescue StandardError
  b
rescue *BAR
  c
end
