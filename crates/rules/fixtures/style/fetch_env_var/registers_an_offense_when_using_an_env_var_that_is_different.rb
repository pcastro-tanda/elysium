if ENV['X']
  puts ENV['Y']
       ^^^^^^^^ Use `ENV.fetch('Y', nil)` instead of `ENV['Y']`.
end
