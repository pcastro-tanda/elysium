ApplicationRecord.writable_transaction do
  loop do
    return if condition
    ^^^^^^ Exit statement `return` is not allowed. Use `raise` (rollback) or `next` (commit).
  end
end
