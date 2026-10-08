ApplicationRecord.writable_transaction do
  throw if user.active?
  ^^^^^ Exit statement `throw` is not allowed. Use `raise` (rollback) or `next` (commit).
end
