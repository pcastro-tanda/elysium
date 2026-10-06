ApplicationRecord.transaction do
  foo.each do
    break if it
  end
end
