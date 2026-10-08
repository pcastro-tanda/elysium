if ENV['X']
  puts ENV.fetch('Y', nil)
end
