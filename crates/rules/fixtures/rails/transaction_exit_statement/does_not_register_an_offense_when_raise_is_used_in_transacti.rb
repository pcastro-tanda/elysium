ApplicationRecord.transaction do
  raise if user.active?
end
