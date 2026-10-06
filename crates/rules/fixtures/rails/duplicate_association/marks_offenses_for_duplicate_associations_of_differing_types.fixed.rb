class Post < ApplicationRecord
  has_and_belongs_to_many :foos, -> { active }
  has_and_belongs_to_many :bars
  has_and_belongs_to_many :blahs

  belongs_to :author
  has_one :top_comment, -> { order(likes: :desc) }, class_name: 'Comment'
end
