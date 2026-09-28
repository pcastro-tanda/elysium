transaction do
  return unless update_necessary?
  items.each do |item|
    return if item.nil?
    ^^^^^^ Non-local exit from iterator, without return value. `next`, `break`, `Array#find`, `Array#any?`, etc. is preferred.
    item.with_lock do
      return if item.stock == 0
      ^^^^^^ Non-local exit from iterator, without return value. `next`, `break`, `Array#find`, `Array#any?`, etc. is preferred.
      item.very_complicated_update_operation!
    end
  end
end
