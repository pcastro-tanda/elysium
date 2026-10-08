class Post < ApplicationRecord
  has_one :foo
  has_one :bar
  has_one :blah
end
