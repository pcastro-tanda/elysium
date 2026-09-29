def main
  raise
rescue StandardError => error
                        ^^^^^ Use `e` instead of `error`.
  error, foo = 1, error
  puts error
end
