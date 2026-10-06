ApplicationRecord.transaction do
  return if user.active?
end
