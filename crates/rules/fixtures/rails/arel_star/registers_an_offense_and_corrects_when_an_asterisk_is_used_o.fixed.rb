class MyModel < ApplicationRecord
  scope :my_scope, -> { select(arel_table[Arel.star]) }
end
