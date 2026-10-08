ApplicationRecord.writable_transaction do
  until stop_looping? do
    break if condition
  end
end
