[1, 2, 3].
  each do |el|
    el.to_s
  rescue StandardError => _exception
    next
  end
