y || ENV['X']
     ^^^^^^^^ Use `ENV.fetch('X', nil)` instead of `ENV['X']`.

y || z || ENV['X']
          ^^^^^^^^ Use `ENV.fetch('X', nil)` instead of `ENV['X']`.
