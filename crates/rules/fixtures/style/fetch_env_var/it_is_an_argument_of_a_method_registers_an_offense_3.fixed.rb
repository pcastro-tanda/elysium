some_method(
  ENV['A'].some_method,
  ENV['B'] || ENV.fetch('C', nil),
  ENV.fetch('X', nil),
  ENV.fetch('Y', nil)
)
