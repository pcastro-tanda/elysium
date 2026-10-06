ApplicationRecord.with_lock do
  break if user.active?
  ^^^^^ Exit statement `break` is not allowed. Use `raise` (rollback) or `next` (commit).
end
