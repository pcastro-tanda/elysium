class Post < ApplicationRecord
  has_one :foo
  ^^^^^^^^^^^^ Association `foo` is defined multiple times. Don't repeat associations.
  has_one :bar
  has_one :foo
  ^^^^^^^^^^^^ Association `foo` is defined multiple times. Don't repeat associations.
  has_one :blah
end
