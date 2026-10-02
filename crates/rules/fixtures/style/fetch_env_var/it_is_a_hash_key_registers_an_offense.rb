{
  ENV['X'] => :x,
  ^^^^^^^^ Use `ENV.fetch('X', nil)` instead of `ENV['X']`.
  ENV['Y'] => :y
  ^^^^^^^^ Use `ENV.fetch('Y', nil)` instead of `ENV['Y']`.
}
