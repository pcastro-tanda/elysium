ApplicationRecord.transaction do
  next if user.active?
end
