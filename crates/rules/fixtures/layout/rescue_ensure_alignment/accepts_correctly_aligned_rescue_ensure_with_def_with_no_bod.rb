def foo
rescue StandardError
  handle_error
ensure
  error
end
