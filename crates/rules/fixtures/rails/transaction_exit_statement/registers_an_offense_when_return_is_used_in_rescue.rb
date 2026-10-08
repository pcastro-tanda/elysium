ApplicationRecord.transaction do
rescue
  return do_something
  ^^^^^^^^^^^^^^^^^^^ Exit statement `return` is not allowed. Use `raise` (rollback) or `next` (commit).
end
