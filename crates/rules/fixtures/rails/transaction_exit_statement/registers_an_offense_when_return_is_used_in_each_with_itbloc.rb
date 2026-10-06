ApplicationRecord.transaction do
  foo.each do
    return if it
    ^^^^^^ Exit statement `return` is not allowed. Use `raise` (rollback) or `next` (commit).
  end
end
