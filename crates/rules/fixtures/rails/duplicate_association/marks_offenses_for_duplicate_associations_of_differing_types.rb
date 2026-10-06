class Post < ApplicationRecord
  has_many :foos
  ^^^^^^^^^^^^^^ Association `foos` is defined multiple times. Don't repeat associations.
  has_and_belongs_to_many :foos
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `foos` is defined multiple times. Don't repeat associations.
  has_and_belongs_to_many :bars
  has_and_belongs_to_many :foos, -> { active }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `foos` is defined multiple times. Don't repeat associations.
  has_and_belongs_to_many :blahs

  has_one :author
  ^^^^^^^^^^^^^^^ Association `author` is defined multiple times. Don't repeat associations.
  has_one :top_comment, -> { order(likes: :desc) }, class_name: 'Comment'
  belongs_to :author
  ^^^^^^^^^^^^^^^^^^ Association `author` is defined multiple times. Don't repeat associations.
end
