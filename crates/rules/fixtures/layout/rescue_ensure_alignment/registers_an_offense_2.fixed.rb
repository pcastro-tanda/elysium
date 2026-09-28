@instance = [1, 2, 3].map do |el|
rescue StandardError => _exception
end
