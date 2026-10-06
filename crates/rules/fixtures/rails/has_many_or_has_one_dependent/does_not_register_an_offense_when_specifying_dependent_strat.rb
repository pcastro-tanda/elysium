class Person < ApplicationRecord
  has_one :foo, dependent: :destroy
end
