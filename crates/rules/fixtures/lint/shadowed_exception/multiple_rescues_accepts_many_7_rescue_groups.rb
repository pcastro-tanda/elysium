begin
  something
rescue StandardError
  handle_error
rescue ErrorA
  handle_error
rescue ErrorB
  handle_error
rescue ErrorC
  handle_error
rescue ErrorD
  handle_error
rescue ErrorE
  handle_error
rescue ErrorF
  handle_error
end
