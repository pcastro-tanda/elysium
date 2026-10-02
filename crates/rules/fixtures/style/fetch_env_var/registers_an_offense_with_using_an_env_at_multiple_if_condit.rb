if a || b && c
  puts ENV['X']
       ^^^^^^^^ Use `ENV.fetch('X', nil)` instead of `ENV['X']`.
end
