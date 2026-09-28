def foo
  something
rescue StandardError
  handle_error
ensure
  error
end
