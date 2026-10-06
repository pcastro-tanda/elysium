class Post < ApplicationRecord
  has_one :foo, -> { active }
  has_one :bar
  has_one :blah
end
