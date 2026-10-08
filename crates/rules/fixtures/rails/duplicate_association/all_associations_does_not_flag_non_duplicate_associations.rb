        class Post < ApplicationRecord
          belongs_to :user

          has_many :comments
          has_many :commenters, through: :comments

          has_many :active_comments, -> { active }, class_name: 'Comment'

          has_one :top_comment, -> { order(likes: :desc) }, class_name: 'Comment'

          has_and_belongs_to_many :related_posts, class_name: 'Post'
        end
