ApplicationRecord.transaction do
  loop do
    break if condition
  end
end
