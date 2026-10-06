ApplicationRecord.with_lock do
  until stop_looping? do
    break if condition
  end
end
