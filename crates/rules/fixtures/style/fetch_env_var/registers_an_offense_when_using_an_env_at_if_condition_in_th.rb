if a == b
  ENV['X']
  ^^^^^^^^ Use `ENV.fetch('X', nil)` instead of `ENV['X']`.
end
