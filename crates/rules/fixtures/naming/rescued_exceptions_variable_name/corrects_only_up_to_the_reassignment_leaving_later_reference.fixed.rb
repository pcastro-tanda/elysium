begin
  do_something
rescue StandardError => e
  error = build_message(e)
end
puts error
