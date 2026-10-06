class Person < ApplicationRecord
  has_many :foo, dependent: nil
end
