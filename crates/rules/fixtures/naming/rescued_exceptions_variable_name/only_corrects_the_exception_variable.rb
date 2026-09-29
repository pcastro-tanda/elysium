def main
  raise
rescue StandardError => error
                        ^^^^^ Use `e` instead of `error`.
  message = error.message
  puts message
end
