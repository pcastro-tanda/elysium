class Post < ApplicationRecord
  belongs_to :foo
  ^^^^^^^^^^^^^^^ Association `foo` is defined multiple times. Don't repeat associations.
  belongs_to :bar
  belongs_to :foo, -> { active }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `foo` is defined multiple times. Don't repeat associations.
  belongs_to :blah
end
