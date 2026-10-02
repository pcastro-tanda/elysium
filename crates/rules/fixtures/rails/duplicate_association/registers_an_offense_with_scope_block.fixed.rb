class Post < ApplicationRecord
  has_and_belongs_to_many :foos, -> { active }
  has_and_belongs_to_many :bars
  has_and_belongs_to_many :blahs
end
