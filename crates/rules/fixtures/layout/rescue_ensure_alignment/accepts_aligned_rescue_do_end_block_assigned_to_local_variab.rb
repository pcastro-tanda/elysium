result = [1, 2, 3].map do |el|
  el.to_s
rescue StandardError => _exception
  next
end
