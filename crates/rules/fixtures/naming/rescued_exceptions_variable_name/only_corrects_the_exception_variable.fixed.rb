def main
  raise
rescue StandardError => e
  message = e.message
  puts message
end
