class MyModel < ApplicationRecord
  scope :my_scope, -> { select(arel_table["*"]) }
                                          ^^^ Use `Arel.star` instead of `"*"` for expanded column lists.
end
