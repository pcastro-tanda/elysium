ApplicationRecord.with_lock do
  foo.each do
    break if _1
  end
end
