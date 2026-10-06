ApplicationRecord.writable_transaction do
  next if user.active?
end
