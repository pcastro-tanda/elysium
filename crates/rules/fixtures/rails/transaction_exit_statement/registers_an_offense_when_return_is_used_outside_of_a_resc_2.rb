ApplicationRecord.with_lock do
  return if user.active?
  ^^^^^^ Exit statement `return` is not allowed. Use `raise` (rollback) or `next` (commit).
rescue
  pass
end
