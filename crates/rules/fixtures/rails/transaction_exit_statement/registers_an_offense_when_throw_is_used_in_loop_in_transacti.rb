ApplicationRecord.transaction do
  loop do
    throw if condition
    ^^^^^ Exit statement `throw` is not allowed. Use `raise` (rollback) or `next` (commit).
  end
end
