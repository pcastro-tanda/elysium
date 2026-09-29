def main
  raise
rescue StandardError => error
                        ^^^^^ Use `e` instead of `error`.
  error = {
    error_message: error.message
  }
  puts error
end
