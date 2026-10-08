if foo? || ENV["X"]
  puts ENV.fetch("X")
end
if ENV["Y"] || bar?
  puts ENV.fetch("Y")
end
if foo? && ENV["X"]
  puts ENV.fetch("X")
end
if ENV["Y"] && bar?
  puts ENV.fetch("Y")
end
