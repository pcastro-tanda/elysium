ApplicationRecord.writable_transaction do
  foo.each do
    return if _1
    ^^^^^^ Exit statement `return` is not allowed. Use `raise` (rollback) or `next` (commit).
  end
end
