ApplicationRecord.writable_transaction do
  while proceed_looping? do
    break if condition
  end
end
