def main
  raise
rescue StandardError => e
  error, foo = 1, e
  puts error
end
