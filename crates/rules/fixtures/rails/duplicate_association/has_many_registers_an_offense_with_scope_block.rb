class Post < ApplicationRecord
  has_many :foos
  ^^^^^^^^^^^^^^ Association `foos` is defined multiple times. Don't repeat associations.
  has_many :bars
  has_many 'foos', -> { active }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `foos` is defined multiple times. Don't repeat associations.
  has_many :blahs
end
