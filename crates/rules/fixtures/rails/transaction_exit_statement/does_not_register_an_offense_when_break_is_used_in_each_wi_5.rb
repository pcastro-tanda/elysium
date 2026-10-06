ApplicationRecord.writable_transaction do
  foo.each do
    break if _1
  end
end
