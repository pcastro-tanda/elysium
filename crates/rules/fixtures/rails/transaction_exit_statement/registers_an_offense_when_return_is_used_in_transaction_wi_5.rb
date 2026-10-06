ApplicationRecord.writable_transaction do
  _1.after_commit { }
  return if user.active?
  ^^^^^^ Exit statement `return` is not allowed. Use `raise` (rollback) or `next` (commit).
end
