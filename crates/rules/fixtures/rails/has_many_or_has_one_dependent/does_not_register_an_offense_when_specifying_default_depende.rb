class Person < ApplicationRecord
  has_one :foo, dependent: nil
end
