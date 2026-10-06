ApplicationRecord.with_lock do
  raise if user.active?
end
