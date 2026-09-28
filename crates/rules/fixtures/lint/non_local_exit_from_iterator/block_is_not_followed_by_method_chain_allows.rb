transaction do
  return unless update_necessary?
  find_each do |item|
    return if item.stock == 0 # false-negative...
    item.update!(foobar: true)
  end
end
