class Post < ApplicationRecord
  has_many 'foos', -> { active }
  has_many :bars
  has_many :blahs
end
