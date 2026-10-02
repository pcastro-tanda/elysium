class Post < ApplicationRecord
  has_many :foos
  has_many :bars
  has_many :blahs
end
