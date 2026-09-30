def test
  # rubocop:push
  # rubocop:disable Style/RescueStandardError
rescue => e
  print e
end
# rubocop:pop
