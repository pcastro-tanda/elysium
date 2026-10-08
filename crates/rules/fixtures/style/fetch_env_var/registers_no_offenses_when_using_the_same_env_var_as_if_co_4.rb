if ENV["X"].present?
  puts ENV["X"]
end
if ENV["X"].in?(%w[A B C])
  puts ENV["X"]
end
if %w[A B C].include?(ENV["X"])
  puts ENV["X"]
end
if ENV.key?("X")
  puts ENV["X"]
end
