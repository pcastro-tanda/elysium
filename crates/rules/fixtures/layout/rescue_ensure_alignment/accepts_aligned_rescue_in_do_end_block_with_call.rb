foo.() do |el|
  el.to_s
rescue StandardError => _exception
  next
end
