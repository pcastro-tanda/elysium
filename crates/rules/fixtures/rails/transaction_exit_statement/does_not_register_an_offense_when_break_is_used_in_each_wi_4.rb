ApplicationRecord.with_lock do
  foo.each do
    break if it
  end
end
