class Person < ApplicationRecord
  has_one :foo, through: :bar
end
