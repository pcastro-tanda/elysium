class Model < ApplicationRecord
  where(record: [record1, record2]).each(&:touch)
                                    ^^^^ Use `find_each` instead of `each`.
end
