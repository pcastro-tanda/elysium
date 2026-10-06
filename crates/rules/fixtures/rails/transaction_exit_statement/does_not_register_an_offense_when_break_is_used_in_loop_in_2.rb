ApplicationRecord.with_lock do
  loop do
    break if condition
  end
end
