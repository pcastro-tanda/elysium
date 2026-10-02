some_method(
  ENV['A'].some_method,
  ENV['B'] || ENV['C'],
              ^^^^^^^^ Use `ENV.fetch('C', nil)` instead of `ENV['C']`.
  ENV['X'],
  ^^^^^^^^ Use `ENV.fetch('X', nil)` instead of `ENV['X']`.
  ENV['Y']
  ^^^^^^^^ Use `ENV.fetch('Y', nil)` instead of `ENV['Y']`.
)
