class Person < ApplicationRecord
  with_options dependent: :destroy do |model|
    model.has_one :foo
  end
end
