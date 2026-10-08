class Person < ApplicationRecord
  with_options dependent: :destroy do
    has_many :foo
  end
end
