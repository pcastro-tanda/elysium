ApplicationRecord.with_lock do
  next if user.active?
end
