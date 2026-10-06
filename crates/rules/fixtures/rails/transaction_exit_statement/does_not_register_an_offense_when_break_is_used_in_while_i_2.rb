ApplicationRecord.with_lock do
  while proceed_looping? do
    break if condition
  end
end
