class Post < ApplicationRecord
  has_and_belongs_to_many :foos
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `foos` is defined multiple times. Don't repeat associations.
  has_and_belongs_to_many :bars
  has_and_belongs_to_many :foos, -> { active }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `foos` is defined multiple times. Don't repeat associations.
  has_and_belongs_to_many :blahs
end
