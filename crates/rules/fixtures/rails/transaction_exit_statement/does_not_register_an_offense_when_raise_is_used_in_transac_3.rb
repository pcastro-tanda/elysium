ApplicationRecord.writable_transaction do
  raise if user.active?
end
