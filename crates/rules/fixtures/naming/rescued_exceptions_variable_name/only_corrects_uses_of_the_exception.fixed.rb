def main
  raise
rescue StandardError => e
  error = {
    error_message: e.message
  }
  puts error
end
