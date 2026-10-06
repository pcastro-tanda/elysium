class Post < ApplicationRecord
  has_many :foos, if: condition, class_name: 'Foo'
  has_many :bars, if: some_condition, class_name: 'Foo'

  has_one :baz, -> { condition }, class_name: 'Bar'
  has_one :qux, -> { some_condition }, class_name: 'Bar'

  belongs_to :group, class_name: 'IndustryGroup', foreign_key: :industry_group_id
end
